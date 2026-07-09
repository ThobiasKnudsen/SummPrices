# SummPrices — Design & Architecture

> Canonical design context for SummPrices. Read this first. It records **what we're building and why**, the **decisions made** (with rationale), and the **open items**. The existing repo code and any older "spec" documents are **out of date** relative to this file — this file wins.
>
> Last updated: 2026-07-09.

---

## 1. Product

**SummPrices** is a **personal "everything you buy" archive**. A user scans (or uploads) *any* receipt — groceries, furniture, electronics, a restaurant bill — and the app stores **the receipt image itself + structured line items**. Core consumer value:

- **Personal history & insight** — look back over time ("where did my money go"), filter by shop, by item, by date range; count how many times item Y was bought from START→END.
- **Credit for contributing** — scanning receipts earns account credit. Viewing your *own* purchases is always free (see §7.7).
- **Price search** — spend credit to query the *crowd / aggregated* price data via the Price API (see §7.7).
- **Digital receipts** — import machine-readable receipts (PDF), not only photos.
- **Export** — select receipts and export them.
- **Later:** "am I overpaying vs other stores?" — unlocked once enough crowd data exists.

**It is not** just groceries, and **not** a per-store tool — it's a universal archive of a person's purchases.

## 2. Positioning & business model

- **Consumer-first, B2B-later.** The free consumer app is the **data-acquisition funnel**; the anonymized aggregate **price index is the future monetizable asset** (sold via the B2B API/dashboard). Build the consumer app + the anonymized price pipeline now; design so the B2B API is added later without a rewrite.
- **The Price API is the single monetization surface.** It serves only the *crowd / aggregated* price data — **never** a user's own receipts, which are always free (see §7.7). Consumers pay in *earned credits*; B2B customers (later) pay in *money*. Same underlying API, different auth/metering.

## 3. Target market

- **Norway-first** (NOK, MVA/VAT, Norwegian chains and receipt formats), **international-ready by design** (country-aware schema, currency per receipt/price, locale-aware parsing).

## 4. Architecture principles

1. **Modular monolith**, not microservices. A Cargo **workspace of libraries + one axum binary**. Extract a service only when load/release cadence actually diverges. (The old spec's 3-microservice split is premature for a solo team.)
2. **Two data domains, separated from day one** (§7):
   - **Operational / PII** — users, receipts, images, their line-item transactions. Tied to identity.
   - **Anonymized price time-series** — item × price × store × time, **no user identity**. Survives GDPR account deletion; is the B2B asset.
3. **Thin client.** Flutter app = capture + upload + display + API calls. **No meaningful processing on the client** — Postgres, the backend, and the extraction service do the work.
4. **Extraction behind a `ReceiptExtractor` trait** — the model/provider is a swappable implementation detail (§6).
5. **GDPR-first** (§8). Self-hosting the model and keeping all data in the EU is a deliberate compliance + product advantage.
6. **Async by default.** Receipt extraction runs off the request path via a durable job queue.

## 5. System architecture

```
Flutter client ──HTTPS──> axum backend (modular monolith) ──> PostgreSQL
   (thin: capture,             │  ├─ identity/auth               │  (operational/PII
    upload, display,           │  ├─ capture/ingest               │   + reference catalog
    API calls)                 │  ├─ extraction (trait)           │   + anonymized
                               │  ├─ catalog (stores/products)    │   price time-series)
                               │  ├─ price-index / Price API      │
                               │  └─ credits/ledger               │
                               │                                  
                               ├──> Object storage (S3-compatible): receipt images
                               └──> Extraction service: self-hosted VLM on on-demand EU GPU
                                     (Ollama/vLLM, OpenAI-compatible localhost endpoint)
```

- **Backend:** Rust, axum 0.8, sqlx 0.8 (Postgres, compile-time-checked), argon2 + JWT auth, `rust-s3` for object storage.
- **Client:** one Flutter app (multi-platform).
- **Object storage:** S3-compatible; receipt images keyed per user; presigned URLs for display.

## 6. Receipt extraction pipeline

**Goal:** receipt image (or digital PDF) → validated structured JSON:
`{ store, org_no, purchase_datetime, currency, line_items[{desc, qty, unit_price, line_total, category?}], subtotal, mva_lines[{rate, base, vat}], total }`.

**Tiered flow (behind `ReceiptExtractor`):**
1. **Structured import first** where possible — a **digital PDF with a text layer** is parsed directly (no OCR). *Manual PDF upload is a launch feature; email/mailbox ingestion is later.*
2. **VLM extraction** for images — a self-hosted vision-LLM takes the image and emits the JSON schema directly.
3. **Validators (Rust)** — normalize NOK (comma decimals, space/period thousands), parse `DD.MM.YYYY`, reconcile the MVA table, handle `pant`/`rabatt` lines, capture the `NO…MVA` org-number as store identity.
4. **Confidence gate (free):** `line_total == qty×unit_price`, `Σ line_totals == subtotal`, `subtotal + MVA == total`, org-number mod-11 checksum. **Pass → store; fail → flag `needs_review`** and/or escalate to a larger model.

**Model choice (verified 2026):**
- **Recommended: Qwen3-VL-Instruct (Apache-2.0)** — start at **4B**, upgrade to **8B** if 4B underperforms on messy Norwegian receipts. General instruction-following VLM → emits our exact JSON schema directly. OCR expanded to 32 languages (helps Norwegian); robust to blur/tilt.
- **AVOID (license):** `Qwen2.5-VL-3B` and **all Nanonets-OCR** models — Qwen *Research* license = **non-commercial**.
- **Sizes:** 8B ≈ 17 GB weights, ~18–20 GB VRAM fp16 (fits a 24 GB card: RTX 4090 / L4 / A10) or ~8–11 GB at 4-bit (16 GB card); 4B ≈ 8 GB fp16 / ~3.5 GB 4-bit. **Cap image `max_pixels`** to avoid OOM.
- OCR-only specialists (PaddleOCR-VL-0.9B, dots.ocr, PP-OCRv5) output page text/markdown, not our schema — optional as a cheap pre-filter or a VRAM-saving 2-stage path, not the primary.

**Serving & deployment:**
- **Serving:** **Ollama** for the MVP (single binary, OpenAI-compatible endpoint, native `json_schema` structured output) → migrate to **vLLM** (guided-JSON + continuous batching) at volume. Backend calls it over `localhost` via `reqwest`. Enforce JSON with constrained decoding; validate server-side before any DB write.
- **GPU deployment: on-demand / scale-to-zero EU GPU.** Batch-drain the queue in warm windows. ~1k receipts/mo ≈ €2–3/mo; 10k/mo ≈ €25–30/mo. EU-sovereign per-second GPU (**Scaleway L4** Paris/Warsaw preferred; RunPod/Modal EU regions with a signed DPA). Migrate to an **always-on Hetzner** GPU (~€184/mo) only above ~66k receipts/mo. **Avoid fly.io** (GPUs deprecated after 2026-08-01).
- **Job mechanism:** durable **Postgres `SELECT … FOR UPDATE SKIP LOCKED`** queue + background worker, so scans survive restarts and the GPU can batch-drain. (The repo's current OCR seam is fire-and-forget `tokio::spawn` + lazy polling — to be upgraded.)

**Non-negotiable before locking a model:** no model has a published **Norwegian-receipt benchmark**. Build a **~50–100 real Norwegian receipt eval set** (Rema/Kiwi/Coop + restaurant/furniture/electronics, incl. faded thermal) and measure line-item / MVA / total accuracy first.

## 7. Data model

> **Star schema.** One big central **fact table** (`transactions` — every line item bought) surrounded by small **dimension tables** (`users`, `chains`, `stores`, `products`, `categories`) that it points to via foreign keys. A separate **price time-series** (`price_history` + `current_prices`) holds price-per-item-per-shop over time. Types are PostgreSQL; fixed-value columns use native `ENUM` types (§7.0). `PK` = primary key, `FK→x` = foreign key to table `x`. The fact table holds the FKs; dimension tables never carry a transaction id.

### 7.0 Enum types

| Enum type | Values |
|---|---|
| `receipt_source` | camera_photo, image_upload, pdf_upload, ereceipt_api |
| `extraction_status` | pending, queued, processing, done, failed, needs_review |
| `item_type` | product, deposit, discount, fee, rounding, unknown |
| `fraud_status` | ok, suspected, confirmed, dismissed |
| `ledger_reason` | scan_reward, price_query, signup_bonus, referral, adjustment, reversal |
| `mapping_status` | proposed, approved, rejected |

### 7.1 Dimension tables (small, shared)

**`users`** — accounts / auth *(extends existing; per-user PII)*

| Column | Type | Key / Rules | Notes |
|---|---|---|---|
| id | UUID | PK | existing |
| email | TEXT | NOT NULL, UNIQUE | existing |
| password_hash | TEXT | NOT NULL | existing (argon2) |
| display_name | TEXT | | existing |
| credit_balance | INT | NOT NULL, default 0 | cached; `credit_ledger` is source of truth |
| trust_score | REAL | NOT NULL, default 0 | anti-fraud reputation; grows with verified scans |
| consent_version | TEXT | | GDPR: privacy/ToS version accepted |
| consent_at | TIMESTAMPTZ | | when consent was given |
| created_at / updated_at | TIMESTAMPTZ | NOT NULL, default now() | existing |

**`chains`** — retail chains (groups stores)

| Column | Type | Key / Rules | Notes |
|---|---|---|---|
| id | UUID | PK | |
| name | TEXT | NOT NULL, UNIQUE | 'Rema 1000', 'Kiwi', 'Coop Extra', … |
| country_code | CHAR(2) | NOT NULL, default 'NO' | |
| created_at | TIMESTAMPTZ | NOT NULL, default now() | |

**`stores`** — one row per physical outlet

| Column | Type | Key / Rules | Notes |
|---|---|---|---|
| id | UUID | PK | |
| chain_id | UUID | FK→chains | NULL for independent shops |
| name | TEXT | NOT NULL | plain text (Nominative Fair Use) |
| org_no | TEXT | | Norwegian org number (outlet / legal entity) |
| country_code | CHAR(2) | NOT NULL, default 'NO' | |
| address / city / postal_code | TEXT | | from the receipt when present |
| latitude / longitude | DECIMAL(9,6) | | OSM geo |
| timezone | TEXT | | IANA tz (e.g. 'Europe/Oslo') — used to compute `purchase_at` (§7.4) |
| osm_id | TEXT | | |
| created_at | TIMESTAMPTZ | NOT NULL, default now() | |

*Indexes:* `chain_id`; `(latitude, longitude)`.

**`products`** — the item catalog; identified by barcode

| Column | Type | Key / Rules | Notes |
|---|---|---|---|
| id | UUID | PK | surrogate key |
| gtin | TEXT | UNIQUE | **the universal number** — EAN/UPC barcode; NULL if item has no barcode |
| name | TEXT | NOT NULL | |
| brand | TEXT | | |
| category_id | INT | FK→categories | |
| created_at | TIMESTAMPTZ | NOT NULL, default now() | |

Note: `gtin` is the universal item id when a barcode exists; many receipt lines (and non-grocery items) have none, so we keep a surrogate `id` too.

**`categories`** — spend categories (hierarchy)

| Column | Type | Key / Rules | Notes |
|---|---|---|---|
| id | INT | PK (identity) | |
| parent_id | INT | FK→categories | hierarchy (NULL = top level) |
| slug | TEXT | NOT NULL, UNIQUE | |
| name | TEXT | NOT NULL | seeded: groceries, dining, furniture, electronics, transport, … |

### 7.2 Fact tables

**`receipts`** — one row per uploaded / scanned receipt (the *header*; parent of the line items)

| Column | Type | Key / Rules | Notes |
|---|---|---|---|
| id | UUID | PK | |
| user_id | UUID | FK→users, NOT NULL, cascade delete | owner |
| source | `receipt_source` | NOT NULL | how it arrived |
| original_asset_key | TEXT | | object-storage key of image/PDF; NULL for API imports |
| original_mime | TEXT | | `image/jpeg`, `application/pdf` |
| store_id | UUID | FK→stores | NULL until store resolved |
| store_name_raw | TEXT | | extracted store text; shown even if unresolved |
| purchase_at | TIMESTAMPTZ | | universal instant of purchase (§7.4 timezone rules) |
| capture_timezone | TEXT | | client device tz at upload — VPN-safe fallback for `purchase_at` |
| currency | TEXT | NOT NULL, default 'NOK' | |
| subtotal / mva_total / total | NUMERIC(12,2) | | |
| extraction_status | `extraction_status` | NOT NULL, default 'pending' | pipeline state |
| extraction_engine | TEXT | | model + version, e.g. `qwen3-vl-8b@2026-06` |
| extraction_conf | REAL | | 0–1 |
| needs_review | BOOLEAN | NOT NULL, default false | low-confidence seam |
| raw_extraction | JSONB | | full model output (audit / reprocess) |
| image_phash | BIT(64) | | perceptual hash — near-duplicate images |
| dedup_signature | TEXT | UNIQUE(user_id, dedup_signature) | hash(user, store, date, total, item_count) |
| txn_signature | TEXT | | hash(org_no, purchase_at, total) — cross-user dup (later) |
| fraud_status | `fraud_status` | NOT NULL, default 'ok' | |
| created_at / updated_at | TIMESTAMPTZ | NOT NULL, default now() | |

*Indexes:* `(user_id, purchase_at DESC)`; `store_id`; `extraction_status` (partial, active states) for the queue; `txn_signature`.

**`transactions`** — **the central fact table: one row per purchased line item.** Biggest table; references every dimension.

| Column | Type | Key / Rules | Notes |
|---|---|---|---|
| id | BIGSERIAL | PK | compact key for the biggest table |
| receipt_id | UUID | FK→receipts, NOT NULL, cascade delete | parent receipt |
| user_id | UUID | FK→users, NOT NULL, cascade delete | dimension (denormalized for user queries) |
| store_id | UUID | FK→stores | dimension (denormalized from receipt for per-store analytics) |
| product_id | UUID | FK→products | dimension; NULL until resolved (the §4/#4 "unsure" seam) |
| category_id | INT | FK→categories | dimension |
| occurred_at | TIMESTAMPTZ | | denormalized `receipts.purchase_at` — for time queries |
| line_no | INT | | order on the receipt |
| description_raw | TEXT | NOT NULL | exactly as extracted |
| description_clean | TEXT | | normalized for search / matching |
| item_type | `item_type` | NOT NULL, default 'product' | handles `pant` / `rabatt` lines |
| quantity | NUMERIC(12,3) | default 1 | supports weight (kg) |
| unit | TEXT | | 'stk', 'kg', 'l' |
| unit_price | NUMERIC(12,2) | | |
| line_total | NUMERIC(12,2) | | |
| mva_rate | NUMERIC(5,2) | | 25.00 / 15.00 / 12.00 |
| confidence | REAL | | |
| needs_review | BOOLEAN | NOT NULL, default false | |
| created_at | TIMESTAMPTZ | NOT NULL, default now() | |

*Indexes:* `(user_id, description_clean)` for "item Y over time"; `(product_id, occurred_at)`; `(store_id, occurred_at)`; `receipt_id`; `category_id`.

### 7.3 Price time-series (item × physical shop over time — the price index)

De-identified: **no `user_id`, no `receipt_id`.** Written by a background de-identification job from `transactions`. Build the *write path* at launch so the asset accrues from day 1.

**`price_history`** — every price observation, **partitioned by month** on `observed_on` (recent partitions hot; old ones compressed — see note below)

| Column | Type | Key / Rules | Notes |
|---|---|---|---|
| id | BIGSERIAL | part of PK (with `observed_on`) | |
| product_id | UUID | FK→products | NULL if unresolved |
| norm_desc | TEXT | | normalized description when product unresolved |
| store_id | UUID | FK→stores | the *physical shop* |
| chain_id | UUID | FK→chains | denormalized |
| region | TEXT | | coarser than store (k-anonymity) |
| country_code | CHAR(2) | NOT NULL, default 'NO' | |
| unit_price | NUMERIC(12,2) | NOT NULL | |
| currency | TEXT | NOT NULL, default 'NOK' | |
| unit | TEXT | | per stk / kg / l |
| observed_on | DATE | NOT NULL, partition key | coarsened from `occurred_at` (date only) |

*Indexes:* `(product_id, observed_on DESC)`; `(store_id, observed_on DESC)`.

**`current_prices`** — hot rollup: the *latest* price per (product, store). Tiny, always cached, powers the common "what's it cost now?" lookup.

| Column | Type | Key / Rules | Notes |
|---|---|---|---|
| product_id | UUID | FK→products, part of PK | |
| store_id | UUID | FK→stores, part of PK | |
| unit_price | NUMERIC(12,2) | NOT NULL | |
| currency | TEXT | NOT NULL, default 'NOK' | |
| unit | TEXT | | |
| observed_on | DATE | NOT NULL | date of the latest observation |
| updated_at | TIMESTAMPTZ | NOT NULL, default now() | |

*Constraint:* PK `(product_id, store_id)` — one hot row per item×shop, upserted when a newer observation arrives.

**Do we need a separate time-series DB? No — PostgreSQL does this.**
- **Now:** native **monthly range-partitioning** of `price_history` (built-in, zero extensions) + the tiny hot `current_prices` table for the common lookup. Recent months stay hot; the Price API mostly reads `current_prices` and the newest partitions.
- **Later (when volume grows):** add the **TimescaleDB** extension (still Postgres) to turn `price_history` into a hypertable with automatic **compression** of old chunks, **retention** policies, and **continuous aggregates** (pre-rolled daily/weekly series). This is exactly the "old data in a less-aggressive cache" idea, done natively. The table is designed so this upgrade is a drop-in — no app changes.

### 7.4 Timezone handling for `purchase_at`

A paper receipt prints *local* wall-clock time with no zone, but `purchase_at` stores a **universal instant**. Resolution order (VPN-proof — never IP geolocation):
1. **Store address / geo → timezone.** If the receipt gives the shop address (or we've resolved `store_id`), use `stores.timezone`. Most reliable.
2. **Client-reported timezone.** Else use `receipts.capture_timezone` — the device's own timezone/position sent at upload (not IP-based, so a VPN doesn't corrupt it).
3. **Fallback** `Europe/Oslo` (Norway-first) if neither is known; flag `needs_review`.

### 7.5 Support tables

**`credit_ledger`** — append-only; balance = Σ delta

| Column | Type | Key / Rules | Notes |
|---|---|---|---|
| id | BIGSERIAL | PK | ordered |
| user_id | UUID | FK→users, NOT NULL, cascade delete | |
| delta | INT | NOT NULL | `+` earn, `−` spend |
| reason | `ledger_reason` | NOT NULL | |
| ref_type | TEXT | | 'receipt', 'price_query', … |
| ref_id | TEXT | | receipt / query id |
| balance_after | INT | NOT NULL | running balance (audit) |
| created_at | TIMESTAMPTZ | NOT NULL, default now() | |

*Constraint:* `UNIQUE(user_id, ref_id) WHERE reason = 'scan_reward'` → a receipt is rewarded **at most once**.

**`raw_text_mappings`** *(later)* — raw string → product, per store/chain, voted / moderated (the corrected "barcode bridge" — never global first-write-wins)

| Column | Type | Key / Rules | Notes |
|---|---|---|---|
| id | UUID | PK | |
| chain_id | UUID | FK→chains | scope to a chain… |
| store_id | UUID | FK→stores | …or a specific store |
| raw_text | TEXT | NOT NULL | |
| product_id | UUID | FK→products, NOT NULL | |
| status | `mapping_status` | NOT NULL, default 'proposed' | |
| votes | INT | NOT NULL, default 0 | |
| proposed_by | UUID | FK→users | |
| created_at | TIMESTAMPTZ | NOT NULL, default now() | |

**`review_queue`** *(later)* — receipts / items needing resolution. MVP uses the `needs_review` flags; a dedicated table + resolution UX comes later.

### 7.6 Anti-fraud & de-duplication

Because scanning earns spendable credit, dedup/anti-fraud is **layered** (stronger than a single hash):

- **MVP:** `image_phash` + `dedup_signature` (UNIQUE per user) block re-uploads of the same receipt; the §6 arithmetic / MVA / org-no self-checks must pass to earn full credit (fail → `needs_review`, no credit); `credit_ledger` idempotency (one `scan_reward` per receipt) prevents double-crediting; basic per-user scan/credit rate limits.
- **Later:** `txn_signature` catches the *same transaction claimed by different users*; `trust_score` down-weights new / low-reputation users; provisional / "escrow" credit that only settles after checks and is reversible via `reversal` ledger entries if a receipt is flagged after the fact.

### 7.7 Free vs credit-metered (where the credit line falls)

- **Always free — personal domain:** viewing / searching your own `receipts` + `transactions`, your personal analytics, and *your own* price history for *your own* purchases. Looking at your own data is **never** gated.
- **Credit-metered — crowd domain:** queries against `price_history` / `current_prices` (aggregated market prices from everyone) via the Price API — each query writes a `price_query` debit. Later, B2B access is the *same* surface, metered by money instead of earned credits.

## 8. GDPR & compliance (hard constraints)

- Receipts can be **Article 9 special-category** data (pharmacy → health, etc.) → treat the corpus as sensitive; use **explicit consent** as the lawful basis.
- **Self-host the model; keep all data in the EU.** No third-party processor for extraction → no DPA/Schrems exposure. If a cloud fallback is ever used, **EU-only**; **avoid US processors** (EU–US Data Privacy Framework is in acute doubt after a June 2026 US Supreme Court ruling).
- **Deletion** must cascade to receipt images in object storage **and backups**, not just the `users` row. Note: `price_history` is de-identified at write time, so it is unaffected by erasure.
- **Export/portability** (Art. 20) — a full per-user export (transactions + arguably source images) should be one query.
- **Post-deletion retention** of `price_history` is allowed **only with genuine anonymization** (real k-anonymity/aggregation), documented — most naive "anonymization" is reversible pseudonymization and would be non-compliant.

## 9. Norway specifics

- **Formats:** NOK comma decimals (`49,90`), space/period thousands, `DD.MM.YYYY` dates, MVA breakdown table (rates 25 % / 15 % / 12 %, **prices shown gross**), `pant` (deposit) and `Rabatt`/`Trumf` lines, `Totalt`/`Å betale` labels, `NO#########MVA` org-number as store id.
- **Chain digital receipts** (Rema 1000 Æ, Coop Medlem, Trumf → Kiwi/Meny) expose perfect structured line items covering ~97 % of grocery — but via **unofficial, reverse-engineered, ToS-gray** APIs. **Opt-in Phase-3 only, never load-bearing.**
- **EHF/Peppol** is B2B/B2G e-invoicing, **not** consumer receipts — out of scope for ingestion.

## 10. Tech stack

- **Backend:** Rust (edition 2024), axum 0.8, tokio, sqlx 0.8 (Postgres + compile-time checks), argon2, jsonwebtoken, `rust-s3`, reqwest.
- **DB:** PostgreSQL (native monthly partitioning for `price_history`; TimescaleDB later if volume needs it).
- **Client:** Flutter (multi-platform).
- **Extraction:** self-hosted Qwen3-VL via Ollama → vLLM, on an on-demand EU GPU.
- **Object storage:** S3-compatible.

## 11. Roadmap (phased)

**Launch (MVP)**
- Auth; capture/upload (photo **and manual digital-PDF upload**); durable extraction queue + self-hosted VLM; validators + confidence gate + `needs_review`; personal archive with filtering (by shop / item / date range) and spend analytics; de-identification job (`transactions` → `price_history` / `current_prices`); credit ledger (earn on scan); Price API (credit-metered) + basic price search; export; GDPR basics (consent, export, delete-with-cascade).

**Later**
- Email/mailbox digital-receipt ingestion; item-uncertainty **resolution flow** + per-store `raw_text_mappings` (crowd/vote); chain-API opt-in import; "overpaying" comparisons; TimescaleDB for the price series; B2B paid Price API + dashboard; richer product/store identity resolution; international expansion.

## 12. Open items / next steps

1. **Data model finalized (§7).** Implement it: **overwrite** the schema SQL + Rust models/handlers to match (no incremental migrations — no deployment/data yet) and do the workspace restructure.
2. Norwegian **eval set** (~50–100 receipts) to validate Qwen3-VL 4B vs 8B before locking the model.
3. Extraction worker (VLM + durable `SKIP LOCKED` queue) + de-identification job (`transactions` → price series).
4. Price-API contract (filters, credit metering, and the B2B-access seam).
5. Client (Flutter) rework to the thin-client shape.

## 13. Decision log

| Date | Decision | Rationale |
|---|---|---|
| 2026-07-09 | Product = universal personal purchase archive (any receipt), not a grocery/price app first | User's stated primary value: organized history of everything bought |
| 2026-07-09 | Consumer-first; B2B price index designed-for but built later | Consumer app is the data funnel; aggregate data is the asset |
| 2026-07-09 | Modular monolith (workspace + one binary), not microservices | Premature to split for a solo team |
| 2026-07-09 | Self-hosted **Qwen3-VL** (4B→8B, Apache-2.0) on an **on-demand EU GPU** | Best accuracy-per-VRAM that emits our schema; on-demand is cheapest at launch; self-host = clean GDPR |
| 2026-07-09 | Extraction behind a `ReceiptExtractor` trait; Ollama→vLLM; durable Postgres queue | Swappable engine; robust async jobs |
| 2026-07-09 | Two data domains: PII vs anonymized price time-series | Clean GDPR deletion; protect the B2B asset |
| 2026-07-09 | Thin client; backend/Postgres do the work | User decision |
| 2026-07-09 | **Personal archive always free**; credit gates only the crowd/aggregated Price API | Gating a user's own data would anger users |
| 2026-07-09 | Item-uncertainty via a `needs_review` seam + nullable `product_id`; full resolution later | Straight seam to handle unsure items later |
| 2026-07-09 | Manual digital-PDF upload at launch; email integration later | User decision |
| 2026-07-09 | GDPR-first; EU-only; avoid US processors; explicit consent | Receipts can be Art. 9 data; DPF legal uncertainty |
| 2026-07-09 | Credits = integer points; cached `users.credit_balance`, `credit_ledger` authoritative | Simple, fast reads, fully auditable |
| 2026-07-09 | **Star schema:** central `transactions` fact table (one row / line item) + `users`/`chains`/`stores`/`products`/`categories` dimensions | User's model; the fact table holds FKs to dimensions |
| 2026-07-09 | Separate `chains` table; `products` identified by barcode (`gtin`, universal number) | User's model |
| 2026-07-09 | PostgreSQL native **`ENUM`** types for fixed-value columns (not TEXT+CHECK) | User's call |
| 2026-07-09 | Price index = `price_history` (monthly-partitioned time-series) + `current_prices` hot rollup; **stay on Postgres**, TimescaleDB later | User: hot recent / cold old; no separate time-series DB needed |
| 2026-07-09 | `purchase_at` = universal `TIMESTAMPTZ`; timezone from store address/geo → else client device tz (VPN-safe, never IP) | User decision on time/VPN |
| 2026-07-09 | Build `price_history` write path at launch; layered anti-fraud (phash + signatures + arithmetic gate + idempotent ledger) | Accrue the B2B asset from day 1; credit has value so dedup must beat a hash |
