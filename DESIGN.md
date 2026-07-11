# SumPrices — Design & Architecture

> Canonical design context for SumPrices. Read this first. It records **what we're building and why**, the **decisions made** (with rationale), and the **open items**. The existing repo code and any older "spec" documents are **out of date** relative to this file — this file wins.
>
> Last updated: 2026-07-09.

---

## 1. Product

**SumPrices** is a **personal "everything you buy" archive**. A user scans (or uploads) *any* receipt — groceries, furniture, electronics, a restaurant bill — and the app stores **the receipt image itself + structured line items**. Core consumer value:

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
   - **Anonymized crowd/price data** — derived from `transactions` (aggregated, **no user identity**). Materialized into a retained de-identified store only when B2B / retention needs it (§7.3).
3. **Thin client.** The web app (React + TS SPA) = capture + upload + display + API calls. **No meaningful processing on the client** — Postgres, the backend, and the extraction service do the work.
4. **Extraction behind a `ReceiptExtractor` trait** — the model/provider is a swappable implementation detail (§6).
5. **GDPR-first** (§8). Self-hosting the model and keeping all data in the EU is a deliberate compliance + product advantage.
6. **Async by default.** Receipt extraction runs off the request path via a durable job queue.

## 5. System architecture

```
React web app  ──HTTPS──> axum backend (modular monolith) ──> PostgreSQL
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
- **Client:** React + TypeScript SPA (Vite, Tailwind). *(Flutter web MVP was replaced; native mobile revisited later.)*
- **Object storage:** S3-compatible; receipt images keyed per user; presigned URLs for display.

## 6. Receipt extraction pipeline

**Goal:** receipt image (or digital PDF) → validated structured JSON:
`{ store{name,org_no,address,city,postal_code,country_code}, purchase_at, currency, receipt_number, payment{method}, subtotal, total, mva_lines[{rate,base,vat}], line_items[{description, product_code, quantity, unit, shelf_unit_price, unit_price, discount_amount, line_total, item_type, price_type, mva_rate}] }` (the full v2 shape lives in `extraction/hosted_vlm.rs`'s prompt). Key fields are promoted to columns (`receipts.receipt_number`, `transactions.product_code`, …); the whole JSON is kept in `receipts.raw_extraction`.

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
- **Engine (chosen):** the extractor calls **any OpenAI-compatible vision endpoint** — env `EXTRACTOR=hosted`, `VLM_URL`, `VLM_MODEL`, `VLM_API_KEY` (bearer). **Dev = OpenRouter** (one key; benchmark many vision models on real Norwegian receipts to pick the best). **Prod = EU-direct** (Mistral, Paris) before real users — receipts are sensitive; OpenRouter is a US router → not EU-resident. It's a config switch, no code change. `EXTRACTOR=mock` for tests/CI. Self-hosted Qwen3-VL on a rented GPU (below) remains an option.
- **Serving (self-host option):** **Ollama** (single binary, OpenAI-compatible endpoint, native `json_schema` structured output) → migrate to **vLLM** (guided-JSON + continuous batching) at volume. Backend calls it via `reqwest`. Enforce JSON with constrained decoding; validate server-side before any DB write.
- **GPU deployment: on-demand / scale-to-zero EU GPU.** Batch-drain the queue in warm windows. ~1k receipts/mo ≈ €2–3/mo; 10k/mo ≈ €25–30/mo. EU-sovereign per-second GPU (**Scaleway L4** Paris/Warsaw preferred; RunPod/Modal EU regions with a signed DPA). Migrate to an **always-on Hetzner** GPU (~€184/mo) only above ~66k receipts/mo. **Avoid fly.io** (GPUs deprecated after 2026-08-01).
- **Job mechanism:** durable **Postgres `SELECT … FOR UPDATE SKIP LOCKED`** queue + background worker, so scans survive restarts and the GPU can batch-drain. (The repo's current OCR seam is fire-and-forget `tokio::spawn` + lazy polling — to be upgraded.)

**Non-negotiable before locking a model:** no model has a published **Norwegian-receipt benchmark**. Build a **~50–100 real Norwegian receipt eval set** (Rema/Kiwi/Coop + restaurant/furniture/electronics, incl. faded thermal) and measure line-item / MVA / total accuracy first.

### 6.1 Extraction model & cost — benchmarked/researched 2026-07-11

*Refines the pre-benchmark plan above with real measurements. Most of this ships on the `debug` branch (not yet on `main`).*

**Benchmark — 9 real receipts (NO/DK/CH), scored on the reconciliation gate, via OpenRouter:**

| Model | Reconciled to total | Notes |
|---|---|---|
| `google/gemini-2.5-pro` | **8/9** | best; fixes the mix-discount, `KORR`-correction, and price-column-misalignment failures the smaller models make |
| `qwen/qwen-2.5-vl-72b-instruct` | ~5/9 | best **open-weight**; matches gpt-4o |
| `openai/gpt-4o` | ~5/9 | |
| `openai/gpt-4o-mini` (earlier default) | worse | misread prices/totals, hallucinated store names |
| `anthropic/claude-3.7-sonnet` | — | **404 on our OpenRouter account** (Anthropic not enabled) |

Tooling: `backend/src/bin/bench_extractors.rs` (score models on the reconciliation metric) and `reprocess_all.rs` (re-extract stored receipts from DB+storage for any account). In-app **debug model picker** (`GET /api/debug/models`, `VLM_MODELS` env) + per-receipt **Rescan** and bulk **Rescan-all** (`POST /api/debug/reprocess-all`) A/B models on live receipts.

**Current state:** default `VLM_MODEL=google/gemini-2.5-pro` (dev, via OpenRouter — **US router, NOT EU-resident → temporary**; move to EU-direct or self-hosted before real users).

**The reconciliation gate is the cost lever (implemented).** `pipeline::compute_review` sums signed line totals vs the printed total (±0.55 rounding) → flags `needs_review` with a reason; the VLM also self-reports `confidence` + `notes`. This turns *accuracy* into *human-review rate* — a cheaper model produces **more flagged receipts, not silent errors** — which de-risks going cheap and makes small self-hosted models viable.

**Cost & self-hosting roadmap (for scale; ~1000 receipts/day = 30k/mo):**
1. **Verify the real bill first.** Observed ~$0.10/receipt is ~8× above gemini-2.5-pro token math (~$0.013/receipt) — likely thinking-tokens + retries + high-res images. It swings the ROI; check the dashboard.
2. **Cheapest win, zero ops — swap the API model:** Gemini 2.5 **Flash** / gpt-4o-mini / a per-token open VLM (DeepInfra ~$0.15/M, Fireworks $0.20/M) → **~$0.005–0.02/receipt**, config-only. May be enough.
3. **Self-host when you want scale savings or data residency (GDPR — receipts are PII):**
   - **Model:** `Qwen3-VL-8B-Instruct` (Apache-2.0, best OCR-per-VRAM) or `Qwen2.5-VL-7B` (safest); `MiniCPM-V 4.5` (tops OCRBench) as alt. Single **24 GB GPU (L4)**, FP8 ≈ 9–10 GB.
   - **Serve** with **vLLM** (OpenAI-compatible, `guided_json` to hard-enforce the schema) → config change on our side (`VLM_URL`/`VLM_MODEL`, keyless). Keep the hosted API as a **hot fallback**.
   - **Always-on, not serverless** (10–60 s cold starts wreck interactive UX). **One always-on L4 ≈ $365/mo ≈ $0.012/receipt**, covers **5–10k receipts/day** → $/receipt *falls* toward ~$0.002 as volume grows (API scales linearly).
   - **Licensing landmines — AVOID:** **Llama-3.2-Vision** (license *carves out EU-based companies* — hard blocker for us), `Qwen2.5-VL-3B` and **Nanonets-OCR** (non-commercial). Use a reputable **EU-region** host with a DPA, not the cheapest marketplace GPU.
4. **End-state — LoRA fine-tune on our own receipts.** The `needs_review` queue is a free labeling machine (`image → our JSON`). ~500–1,000 corrected labels (2–4 weeks at volume) → LoRA-tune Qwen2.5-VL-7B on one 24 GB GPU (~$50, ~1 h). A narrow-domain tune **beats frontier models on our receipts** (learns store headers, MVA/`pant`/`Rabatt`/`KORR`, comma-decimals) → **lower review rate** at ~$0.002/receipt. Re-tune monthly as formats drift. **This is the moat.**

**Skip two-stage OCR→text-LLM for our receipts:** flattening the page loses row/price alignment — the main failure on crumpled thermal receipts (exactly what the prompt's ROW-ALIGNMENT rules fight). Only worth it for clean/PDF receipts or if raw OCR text is needed elsewhere.

**Honest downside of self-hosting:** the GPU bill isn't the cost — devops (vLLM/CUDA upkeep), cold-start/availability, monitoring, on-call, and GDPR host choice are. At 1k/day the ~$2.6k/mo saving can be eaten by ~1 engineer's setup in year one → **at current volume, self-hosting is a bet on scaling, not an instant win.** Sequence: Flash test → always-on Qwen3-VL-8B (API fallback) → harvest labels → LoRA.

## 7. Data model

> **Star schema.** One big central **fact table** (`transactions` — every line item bought) surrounded by small **dimension tables** (`users`, `chains`, `stores`, `products`, `categories`) that it points to via foreign keys. Crowd/price data is **derived from `transactions`** (aggregate queries), not a separate table at MVP (§7.3). Types are PostgreSQL; fixed-value columns use native `ENUM` types (§7.0). `PK` = primary key, `FK→x` = foreign key to table `x`. The fact table holds the FKs; dimension tables never carry a transaction id.

### 7.0 Enum types

| Enum type | Values |
|---|---|
| `receipt_source` | camera_photo, image_upload, pdf_upload, ereceipt_api |
| `extraction_status` | pending, queued, processing, done, failed, needs_review |
| `item_type` | product, deposit, discount, fee, rounding, unknown |
| `fraud_status` | ok, suspected, confirmed, dismissed |
| `ledger_reason` | scan_reward, price_query, signup_bonus, referral, adjustment, reversal, barcode_reward |
| `ledger_settle_state` | pending, settled, reversed |
| `mapping_status` | proposed, approved, rejected |
| `mapping_key_type` | plu, raw_text, ean |
| `resolution_method` | unresolved, exact_ean, plu_map, fuzzy, barcode_scan, consensus, moderated |
| `price_type` | shelf, promo, member, coupon, net_only |

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
| captured_at | TIMESTAMPTZ | | client clock at capture — authenticity provenance (§7.9), un-backfillable |
| capture_meta | JSONB | | client-reported provenance: has-EXIF, EXIF datetime, camera-vs-file-pick, app version, capture→upload latency (§7.9) |
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
| authenticity_score | REAL | | 0–1 statistical authenticity (§7.9); `fraud_status` = its thresholded label |
| fraud_status | `fraud_status` | NOT NULL, default 'ok' | |
| extraction_attempts | INT | NOT NULL, default 0 | retry counter for the queue |
| extraction_error | TEXT | | last failure message |
| next_attempt_at | TIMESTAMPTZ | | backoff time for retries |
| created_at / updated_at | TIMESTAMPTZ | NOT NULL, default now() | |

*Indexes:* `(user_id, purchase_at DESC)`; `store_id`; `extraction_status` (partial, active states) for the queue; `txn_signature`.

The `receipts` table **is** the extraction queue — the worker polls `WHERE extraction_status IN ('pending','queued') … FOR UPDATE SKIP LOCKED`. A generic `jobs` table is only worth it once job types multiply (§7.8).

**`transactions`** — **the central fact table: one row per purchased line item.** Biggest table; references every dimension.

| Column | Type | Key / Rules | Notes |
|---|---|---|---|
| id | BIGSERIAL | PK | compact key for the biggest table |
| receipt_id | UUID | FK→receipts, NOT NULL, cascade delete | parent receipt |
| user_id | UUID | FK→users, NOT NULL, cascade delete | dimension (denormalized for user queries) |
| store_id | UUID | FK→stores | dimension (denormalized from receipt for per-store analytics) |
| product_id | UUID | FK→products | dimension; NULL until resolved (§7.10 cascade) |
| product_code | TEXT | | store's printed article/PLU code (or EAN) — Tier-0/1 resolution key (§7.10) |
| resolution_method | `resolution_method` | NOT NULL, default 'unresolved' | how `product_id` was set (§7.10) |
| resolution_confidence | REAL | | P(match) for a probabilistic (fuzzy) resolution; NULL for deterministic |
| category_id | INT | FK→categories | dimension |
| occurred_at | TIMESTAMPTZ | | denormalized `receipts.purchase_at` — for time queries |
| line_no | INT | | order on the receipt |
| description_raw | TEXT | NOT NULL | exactly as extracted |
| description_clean | TEXT | | normalized for search / matching |
| item_type | `item_type` | NOT NULL, default 'product' | handles `pant` / `rabatt` lines |
| quantity | NUMERIC(12,3) | default 1 | supports weight (kg) |
| unit | TEXT | | 'stk', 'kg', 'l' |
| shelf_unit_price | NUMERIC(12,2) | | shelf/list price per unit *before* discount (when the receipt shows it) |
| unit_price | NUMERIC(12,2) | | **net** price per unit actually paid |
| discount_amount | NUMERIC(12,2) | | line discount = (shelf − net) × qty; 0 / NULL if none |
| line_total | NUMERIC(12,2) | | **net** amount paid for the line |
| price_type | `price_type` | NOT NULL, default 'net_only' | shelf / promo / member / coupon / net_only (§7.3) |
| mva_rate | NUMERIC(5,2) | | 25.00 / 15.00 / 12.00 |
| confidence | REAL | | |
| needs_review | BOOLEAN | NOT NULL, default false | |
| created_at | TIMESTAMPTZ | NOT NULL, default now() | |

*Indexes:* `(user_id, description_clean)` for "item Y over time"; `(product_id, occurred_at)`; `(store_id, occurred_at)`; `receipt_id`; `category_id`.

### 7.3 Crowd / price index — derived, not stored (yet)

**There is no separate price table at MVP.** Every `transactions` row already *is* a price observation (`product_id` / `description_clean`, `store_id`, `occurred_at`, `unit_price`, `unit`, `currency`). A dedicated `price_history` table would be a ~1:1 duplicate of the biggest table, so:

- **Crowd/market price queries = aggregate queries over `transactions`** (grouped by **resolved `product_id`** × store × time — §7.10; unresolved / only-guessed lines don't enter the crowd index), governed by the §7.3.1 risk model and credit-metered at the API (§7.7). The free "*your own* item Y over time" needs **no** resolution — it's a `transactions` query on `(user_id, description_clean)` — but the **crowd** aggregate **requires** product resolution (§7.10), else the same item under N store spellings fragments into N "products."
- **`current_prices` (latest price per product × store)** is *not* 1:1 — it collapses to one row per pair. If/when price search needs speed, add it as a **materialized view** over `transactions` (refresh periodically). Not needed at launch.

**Price semantics — one item can have several prices at once.** Two shoppers can pay different amounts for the same item at the same store+time (member price, coupon, promo), so a price is not a single number. We model it per line from *what the receipt shows*:
- `unit_price` / `line_total` = the **net** the user actually paid — always captured; this is what the personal archive uses.
- `shelf_unit_price` + `discount_amount` = the **store-set** price and the reduction, *when the receipt itemizes them* (a base line + a `Rabatt`/`Trumf` line).
- `price_type` classifies the observation: `shelf` / `promo` are **store-set** (user-independent, comparable across shoppers); `member` / `coupon` are **personal**; `net_only` = we only know what was paid.
- Line-attributable discounts fold onto the product row (shelf + discount); basket-level discounts stay as standalone `item_type = 'discount'` transactions.

For the crowd **price index**, compare **store-set prices** (`shelf` / `promo`) for apples-to-apples; surface `member` prices as a separate tier; never mix a coupon price into the shelf-price series. **We can only model what's on the receipt** — a bare net total is stored tagged `net_only`. This stays general across all shops worldwide while representing the richer cases when the data is there. **Out of per-item scope:** chain loyalty rebates that pay 1–3 % back on the whole basket to a membership account (Coop *kjøpeutbytte*, Trumf) — a basket-level perk paid later, not a per-item price (optionally a receipt-level note if we ever want effective-cost analytics).

**Deferred: a de-identified retained `price_history`.** Its *only* justification is GDPR — a copy with **no `user_id`** that (a) survives a user deleting their account and (b) lets the B2B API answer without touching PII. Build it when B2B/retention actually lands (a background job snapshots `transactions` → de-identified, coarsened, k-anonymized rows). **Trade-off of deferring:** until then, a user who deletes their account removes their contribution from the crowd aggregates — acceptable at MVP scale. The asset still accrues from day 1 *inside `transactions`*.

**Time-series at scale — still just Postgres.** When the retained `price_history` arrives, use native **monthly range-partitioning** (built-in) so recent months stay hot and old ones can be compressed; add the **TimescaleDB** extension later for automatic compression / retention / continuous aggregates (the "old data in a less-aggressive cache" idea). No separate time-series DB needed.

### 7.3.1 Statistical anonymity of the Price API

*Refines the "k-anonymity" note above: **there is no fixed K.** Availability is governed by a re-identification-**risk formula** over the crowd behind each query, plus differential-privacy noise. A user's own receipts are never affected (they always see theirs in full) — this section is only about what a **third party** can pull from the aggregate.*

- **No row-level surface.** The Price API has **no per-receipt / per-transaction endpoint**. The *only* query is: *for item X, give the price (and volume) distribution over an **area range** × **time range** × **price band**.* Individual receipts, baskets, and `user_id`s never leave the PII domain.

- **Per-item, independent cells.** A result is an aggregate over a **cell = (product × area-range × time-range × price-band)**, computed over the item's **entire buyer population** in that cell. Multiple items in one query = a **batch of independent per-item aggregates**, never the intersection of who bought them together. **Hard rule (basket protection): no conjunctive / co-occurrence queries** ("bought A *and* B") and **no join key shared across item results**. Multi-item resolution is a presentation choice: **common** (the rarest item drives one shared coarsest cell, apples-to-apples) or **per-item** (each at its own finest safe cell).

- **Identifiability is computed, not thresholded.** For a cell with **n = distinct buyers** and **m = purchases** of the item:
  - **Isolation risk** (adversary narrows a known target to 1-of-n): `ρ = 1/n`.
  - **Population-uniqueness** (cell effectively backed by one person), Poisson(λ ≈ n): `P(unique | non-empty) = λ·e^(−λ) / (1 − e^(−λ))` — falls exponentially with the crowd (λ=3 → ~16 %, λ=10 → ~0.05 %, λ=20 → ~4×10⁻⁸).
  A cell is answerable **iff its computed risk ≤ one global policy tolerance** (a max re-identification probability). No hard-coded K; crowd size drives everything, and each answer can carry its own risk/confidence.

- **Differential privacy on every released number (price *and* volume).** Cap each user's contribution to a cell (**count once per product-cell** → sensitivity Δ), then release `value + Laplace(Δ/ε)`. ε bounds each individual's influence — `P(output | in) / P(output | out) ≤ e^ε` — and **composes across queries** as a spent **privacy budget per data consumer** (what plain k-anonymity can't do; it's what stops *differencing* attacks from repeated/overlapping queries). ε (or the max-risk tolerance) is the **single, auditable policy dial.**

- **Resolution is a shared budget across the axes.** n is monotone in each axis's width, so specificity is *spent*: pin one axis tight and the others must widen to gather enough crowd — "one specific shop" → the time range auto-expands until enough distinct buyers of that item are present; tighten the price band → time widens further. There's a Pareto frontier of (area, time, price) widths that just clear the gate. **API UX: pin the axis you care about, mark the rest `auto`** → the API returns the finest achievable resolution on the auto axes, or the minimum window needed ("this store needs ≥ 6 weeks"). Price precision scales with the crowd: the band tightens ~`1/√n` above the ε-noise floor; a thin crowd yields a band so wide it says nothing individual, else the query **coarsens** (store→city→region, day→week→month) or **suppresses**.

- **DB impact — little; it's a query-layer + access-control job.** Raw `transactions` already hold everything (product, price, `occurred_at`, store→geo, buyer), so **no core-schema change**, and the personal-archive path is unchanged (a user reads their own rows by `user_id`). Enforce privacy in the **backend query layer** (where the ρ / P_unique + DP math lives — this path must be airtight) **and** reinforce it in Postgres with **a separate DB role for the price path that physically cannot read `user_id` / raw rows** (a view or RLS exposing only gated aggregates), so an app bug can't leak PII. Same rows, **two permission-separated access paths**: *your archive* (you, full detail) vs. *the price signal* (others, gated aggregates only) — the §4/§7 two-domain split realized as **access control, not a second copy**. Optional, for scale not correctness: precomputed **rollup cells** `(product × region × time-bucket) → distinct_buyers, n_purchases, price histogram` (where the contribution cap is enforced) + a **DP-budget ledger** `(consumer, ε spent)`. This is exactly what the deferred de-identified `price_history` (above) becomes when B2B/retention lands.

- **Honest limit.** At the finest end — one store, one day, a rare item — you cannot have both accuracy and privacy (information-theoretic, not a bug); coarsen gracefully or suppress there. B2B buyers rarely need that granularity (they want trends / regional distributions, which aggregate cleanly), so utility lost to the guarantee is small.

- **GDPR bearing.** This is what makes the price domain genuinely **anonymous**, not merely pseudonymous: released stats summarize ≥ "statistically many" people with a *provable* per-person bound (ε), so re-identification is not "reasonably likely" even against a motivated intruder joining auxiliary data. Raw `transactions` stay the PII domain (deletable); only DP aggregates cross the boundary. (Contrast: exact price + exact store + light time-jitter on *rows* would be *pseudonymization* — still personal data, still in scope.)

### 7.4 Timezone handling for `purchase_at`

A paper receipt prints *local* wall-clock time with no zone, but `purchase_at` stores a **universal instant**. Resolution order (VPN-proof — never IP geolocation):
1. **Store address / geo → timezone.** If the receipt gives the shop address (or we've resolved `store_id`), use `stores.timezone`. Most reliable.
2. **Client-reported timezone.** Else use `receipts.capture_timezone` — the device's own timezone/position sent at upload (not IP-based, so a VPN doesn't corrupt it).
3. **Fallback** `Europe/Oslo` (Norway-first) if neither is known; flag `needs_review`.

### 7.5 Support tables

**`refresh_tokens`** — session management (JWT access tokens are short-lived; these rotate/revoke)

| Column | Type | Key / Rules | Notes |
|---|---|---|---|
| id | UUID | PK | |
| user_id | UUID | FK→users, NOT NULL, cascade delete | |
| token_hash | TEXT | NOT NULL, UNIQUE | store a hash, never the raw token |
| expires_at | TIMESTAMPTZ | NOT NULL | |
| revoked_at | TIMESTAMPTZ | | NULL = active |
| created_at | TIMESTAMPTZ | NOT NULL, default now() | |

*Index:* `(user_id)`.

**`credit_ledger`** — append-only; balance = Σ delta

| Column | Type | Key / Rules | Notes |
|---|---|---|---|
| id | BIGSERIAL | PK | ordered |
| user_id | UUID | FK→users, NOT NULL, cascade delete | |
| delta | INT | NOT NULL | `+` earn, `−` spend |
| reason | `ledger_reason` | NOT NULL | |
| settle_state | `ledger_settle_state` | NOT NULL, default 'settled' | escrow seam (§7.9); spendable `credit_balance` = Σ delta WHERE settled |
| ref_type | TEXT | | 'receipt', 'price_query', … |
| ref_id | TEXT | | receipt / query id |
| balance_after | INT | NOT NULL | running balance (audit) |
| created_at | TIMESTAMPTZ | NOT NULL, default now() | |

*Constraint:* `UNIQUE(user_id, ref_id) WHERE reason = 'scan_reward'` → a receipt is rewarded **at most once**.

*Escrow (§7.9):* `scan_reward` posts `settle_state = 'pending'`; a settlement step (trivial/auto at MVP → async statistical batch later) flips it to `settled`, or a `reversal` entry claws it back. Cached `credit_balance` counts only `settled` deltas. Non-scan reasons default straight to `settled`.

**`product_mappings`** — the "barcode bridge": `(scope, key) → product`, from barcode scans / consensus / moderation. Powers the §7.10 resolution cascade (Tiers 1–3). Never global first-write-wins.

| Column | Type | Key / Rules | Notes |
|---|---|---|---|
| id | UUID | PK | |
| chain_id | UUID | FK→chains | scope to a chain… |
| store_id | UUID | FK→stores | …or a specific outlet (NULL = whole chain) |
| key_type | `mapping_key_type` | NOT NULL | plu / raw_text / ean |
| key_value | TEXT | NOT NULL | the PLU code, normalized raw text, or EAN |
| product_id | UUID | FK→products, NOT NULL | |
| method | `resolution_method` | NOT NULL | how it arose: barcode_scan / consensus / moderated / fuzzy |
| confidence | REAL | | §7.9-style consensus confidence |
| status | `mapping_status` | NOT NULL, default 'proposed' | |
| votes | INT | NOT NULL, default 0 | independent corroborations |
| proposed_by | UUID | FK→users | |
| created_at | TIMESTAMPTZ | NOT NULL, default now() | |

*Unique:* `(chain_id, store_id, key_type, key_value)` — one active mapping per key/scope. *Index:* `(chain_id, key_type, key_value)` for the Tier-1 lookup.

**`review_queue`** *(later)* — receipts / items needing resolution. MVP uses the `needs_review` flags; a dedicated table + resolution UX comes later.

### 7.6 Anti-fraud & de-duplication

> The signals below are the **authenticity** axis of the trust model; §7.9 is the full statistical framing (three signals, two gates) that this feeds.

Because scanning earns spendable credit, dedup/anti-fraud is **layered** (stronger than a single hash):

- **MVP:** `image_phash` + `dedup_signature` (UNIQUE per user) block re-uploads of the same receipt; the §6 arithmetic / MVA / org-no self-checks must pass to earn full credit (fail → `needs_review`, no credit); `credit_ledger` idempotency (one `scan_reward` per receipt) prevents double-crediting; basic per-user scan/credit rate limits.
- **Later:** `txn_signature` catches the *same transaction claimed by different users*; `trust_score` down-weights new / low-reputation users; provisional / "escrow" credit that only settles after checks and is reversible via `reversal` ledger entries if a receipt is flagged after the fact.

### 7.7 Free vs credit-metered (where the credit line falls)

- **Always free — personal domain:** viewing / searching your own `receipts` + `transactions`, your personal analytics, and *your own* price history for *your own* purchases. Looking at your own data is **never** gated.
- **Credit-metered — crowd domain:** aggregate queries over everyone's `transactions` (§7.3) via the Price API — each query writes a `price_query` debit. Later, B2B access is the *same* surface, metered by money instead of earned credits.

### 7.8 Tables considered & deferred

Not built at MVP; documented so we don't rediscover the need later:

- **De-identified `price_history` + `current_prices` view** — when B2B / retention-past-deletion / scale arrives (§7.3).
- **Generic `jobs` table** — only if job types multiply beyond extraction (MVP: `receipts` is the queue, §7.2).
- **`consent_events`** — full GDPR consent audit trail (MVP uses columns on `users`).
- **`data_requests`** — track GDPR export / deletion (DSAR) requests and their status.
- **`devices`** — push-notification tokens (when notifications ship).
- **`store_aliases`** — raw store-name → `store_id` resolution (the store-side analog of `product_mappings` §7.10; part of the later identity-resolution flow).
- **`receipt_tags` / notes** — user annotations on receipts.
- **Item-enrichment tables** (`item_contributions`, `info_requests`, `contribution_verifications`) + KYC fields — the crowdsourced-enrichment vision (§14).

### 7.9 Receipt trust — three statistical signals, two gates

*How SumPrices decides how far to trust a receipt. **Trust is not one number.** A receipt can be perfectly read but fabricated, or genuine but misread — collapsing that into a single score destroys the *reason*, which both the review queue and the B2B "price + confidence" claim need. So we compute **three independent, individually-inspectable signals** and let the two value-exits consume them differently. Trust is only enforced where **value crosses a membrane** — earning spendable credit, or a row entering the crowd/Price-API aggregate (§7.3.1); a user's own archive is **never** gated (§7.7).*

**The three signals.** Each is a calibrated probability/score with its own home column:

| Signal | Question / threat | Method (★ = statistical) | Output |
|---|---|---|---|
| **Fidelity** | Did we *read* it right? (VLM error) | Reconciliation residual + field/model confidence → ★ logistic **calibration** | `receipts.extraction_conf`, `transactions.confidence` |
| **Authenticity** | Is it a *genuine record of a real purchase*? (fabrication, tampering, AI-gen, duplication) | ★ Corroboration likelihood-ratio + ★ robust novelty detection + dedup-as-LR | `receipts.authenticity_score` → thresholded to `fraud_status` |
| **Reliability** | How much should it *move the crowd price*? (unrepresentative data, low-rep contributor) | ★ Hierarchical Bayesian truth-discovery + robust weighted aggregation | `users.trust_score` (contributor) + per-observation weight (derived at aggregation) |

**The arithmetic gate proves fidelity, not authenticity.** `Σ(line totals) == total` shows the *transcription is self-consistent* — but a fabricated receipt makes the arithmetic add up *by construction*. Reconciliation says nothing about whether the purchase happened; authenticity is a separate, adversarial problem.

**Evidence combines in log-odds.** Each signal within an axis is a *weight of evidence* `log[P(sig|true)/P(sig|false)]`; the sum is a posterior log-odds (naive-Bayes form) → additive, **inspectable** (which term sank it), **calibrated**, and **graceful under missing signals** (absent = 0 evidence, so cold-start doesn't break). Once the review queue + reversals yield labels, replace hand-set weights with a fitted **logistic/GBM** (relaxes the naive-independence assumption). **Two gates consume the signals differently:**
- **Credit gate** (`scan_reward`): fidelity × authenticity × dedup. Fail → no/partial credit + `needs_review`.
- **Index-inclusion gate** (row counts toward the Price-API aggregate, §7.3.1): authenticity × reliability-weight, fidelity as a precondition. A low-authenticity / low-weight row is down-weighted or excluded from the *crowd*, but still lives in the owner's archive.

**Authenticity — statistical (label trajectory: unsupervised → PU → supervised).** No fraud labels exist at launch, so it starts label-free and matures as the review queue + credit reversals accrue weak labels.
1. **Corroboration LR — the index doing double duty.** Per line, `WoE = log[ f_genuine(price | robust crowd density for product×store×week) / f_null(price | broad category prior) ]`, summed across lines (≈ independent given store+week) → a receipt-level authenticity LR. `f_genuine` = robust density on **log-price** (Student-t / small mixture for promo modes, MAD scale); `f_null` deliberately wide. **Cold-start self-heals:** thin crowd → wide `f_genuine` → WoE ≈ 0 → falls back to prior + rules. The asset you sell *is* the fraud detector, switching itself on as data accrues.
2. **Novelty detection** on receipt-level features (huge genuine class, ~no negatives → one-class): **robust Mahalanobis (MCD covariance) → χ² tail p-value** over capture channel (live camera vs gallery), time-of-day vs store hours, a user's scan inter-arrival (bursts = farming), MVA-table consistency, item-count-vs-total, EAN-resolvability. Upgrade to Isolation Forest later.
3. **Duplicates as LR, not equality.** `image_phash` Hamming + fuzzy `txn_signature` → `log[P(d|same)/P(d|diff)]`. Same-transaction-different-user (`txn_signature`) is the highest-value fraud signal.

Low **fidelity** *gates* authenticity: you cannot judge a receipt you could not read → route to review, never auto-reject.

**Reliability — statistical (the §14 truth-discovery layer, made concrete).**
- **Contributor reliability = *continuous* truth-discovery, not categorical Dawid–Skene.** Prices are continuous, so the DS analogue is a **hierarchical Bayesian measurement model** (CRH / CATD family): latent true price per `store×window` + per-contributor **bias & precision** as random effects, fit by **batch EM** (nightly). `users.trust_score` = estimated precision, earned from agreement-with-inferred-truth over time. It yields a **confidence interval on every aggregate for free** — that CI *is* the B2B product (§7.3.1's "price + confidence").
- **Robust weighted aggregation.** Published crowd price for a cell = **weighted median / Huber M-estimator**, weights = `trust_score × recency-decay × authenticity`. Robustness does double duty: rejects honest outliers (clearance/damaged) *and* its high breakdown point is the **anti-poisoning** defense (a few adversarial rows can't move a weighted median). Stratify by `price_type` (§7.3) — never average shelf against member.
- **KYC = strong prior on precision, not an oracle** (§14): starts a contributor high, still updated by track record, **hard cap on any single actor's weight** (influence-function guardrail against one KYC'd bad actor).

**Fidelity — calibration only (kept light, per the "at least authenticity + reliability" call).** Turn the hard `±0.55` reconciliation threshold into `p_fidelity = σ(β₀ + β₁·|residual| + β₂·min_field_conf + …)` fit by **Platt/isotonic** on the labeled queue; the threshold becomes a **cost-matrix decision** (false-accept cost vs human-review cost). Same labels feed the LoRA flywheel (§6.1).

**One loop, three outputs.** Crowd prices power authenticity's corroboration LR → authenticity weights feed reliability's robust aggregation → reliability's converged truth re-scores contributors → sharper authenticity priors next round.

**Schema seams pulled to MVP** (all additive, nearly free; one *irreversible*):
- **Capture provenance** *(irreversible — un-backfillable)*: the client records `receipts.captured_at` (device clock at capture) + `receipts.capture_meta` JSONB (has-EXIF, EXIF datetime, in-app-camera vs file-pick beyond the `receipt_source` split, app version, capture→upload latency). Nothing consumes it at MVP; without it every pre-launch receipt is permanently featureless for novelty scoring.
- **Escrow credit seam** *(§7.6 "later" — seam now)*: `credit_ledger.settle_state` (`pending`/`settled`/`reversed`); spendable `credit_balance` = Σ delta **where settled**. `scan_reward` posts `pending`; the settlement step (trivial/auto now → async statistical batch later) flips to `settled` or posts a `reversal`. Cheap now, painful to retrofit once credit is live.
- **`receipts.authenticity_score` REAL** — the statistical output; `fraud_status` becomes its thresholded label + human-review outcome. Reliability's per-observation weight is **derived at aggregation** (trust_score × recency × authenticity), not stored — keeps the biggest table lean (consistent with §7.3 "derive, don't duplicate").

**Phasing (statistical maturity, cost-aware).**
- **P0 (MVP):** calibrated fidelity + deterministic sanity (date-not-future, legal MVA rates, sane totals, currency-matches-country, **org-no checked against the Brønnøysund Enhetsregister** — free EU-resident API) + dedup-as-LR. Authenticity = prior + rules; reliability = robust weighted median, flat weights. Escrow + provenance seams present.
- **P1 (crowd fills in):** corroboration LR + robust-Mahalanobis novelty; light EM for contributor precision; `txn_signature` cross-user dedup.
- **P2 (adversarial maturity):** full hierarchical Bayesian truth-discovery w/ CIs; PU→supervised fraud classifier off accumulated labels; KYC priors; collusion-cluster detection; escrow settlement becomes real.

### 7.10 Product resolution — receipt line → universal product (GTIN)

*Receipt line names are cryptic, abbreviated, store-specific (`"H-MELK 1,75"`, `"TINE LETTMLK"`). For the **crowd Price index** (§7.3) to work, the same physical product under N store spellings must collapse to **one** product — else `grouped by product` fragments into garbage. **Personal** single-store analytics survive on `description_clean` alone; the **crowd asset cannot** — so resolution is load-bearing for the asset, not polish. It is structurally the **same truth-discovery problem as §7.9** (each key has a latent true GTIN; barcode scans are high-confidence votes; the matcher is a prior; consensus resolves it) and reuses that machinery (weighted consensus, escrow, trust-weighting).*

**Resolution cascade — strongest/cheapest signal first.** Runs at ingest; re-runnable over history via `reprocess_all` as the matcher improves (so, unlike capture-provenance, the resolver itself *can* be back-applied — only the barcode **labels** benefit from being collected at the natural moment).

1. **Tier 0 — free self-resolution.** When the receipt's printed `product_code` is a valid **EAN-13**, resolve straight to GTIN — no user, no model. (Barcoded goods often print the EAN; weighed goods print an internal PLU.) → `exact_ean`.
2. **Tier 1 — deterministic `(chain_id, product_code)` PLU map. The backbone.** A store PLU is a stable key: once *any* signal resolves `(chain, PLU) → product`, **every future line with that PLU, across all users, resolves with near-certainty** — a `product_mappings` lookup (`key_type = plu`). After a short warm-up this resolves the *majority* of recurring grocery lines, nearly free. → `plu_map`.
3. **Tier 2 — probabilistic string match ("guess with statistics").** No known PLU map → fuzzy/semantic match of `description_clean` (+ brand tokens, qty/unit, price band) against known products → **candidate GTIN + calibrated `P(match)`**. Start with Postgres **`pg_trgm`** trigram similarity (no embeddings infra); calibrate similarity→probability (Platt/isotonic) on the barcode-scan labels — the same calibration trick as §7.9 fidelity. → `fuzzy`.
4. **Tier 3 — barcode scan = gold label, earns credit.** User scans the item barcode → writes a certain `(chain, product_code)→GTIN` **and** `raw_text→GTIN` mapping (`method = barcode_scan`), which **propagates** to all past & future lines sharing that key. → `barcode_scan`.

**The flywheel.** Tier-3 scans are the ground-truth labels that **train & calibrate Tier 2**; Tier-1 PLU maps fall out for free. Same crowd-labels → self-improving-model loop as the LoRA extractor (§6.1) and the trust model (§7.9).

**Credit = value of information, not flat.** Pay **more** to scan an item currently *ambiguous & frequently purchased* (high info-gain), **little** for an already-resolved one — mirrors §14's `demand × difficulty × info-gain`. Maximizes label value **and** removes the farming incentive. `ledger_reason = barcode_reward`, posted through **escrow** (§7.9 `settle_state`).

**A scan is a *claim* — anti-fraud reuses §7.9.** Provisional (escrow) → settles on **independent corroboration** (other users scanning the same PLU → same GTIN, trust-weighted consensus). Cheap sanity gate: fuzzy-match the *scanned GTIN's known product name* against the receipt line — a `"melk"` line + a Coca-Cola barcode → reject/flag. Anti-farming is the same lever as the value-of-information reward.

**The gate (ruling): probabilistic guesses power PERSONAL freely, the CROWD only on corroboration.** A Tier-2 `fuzzy` match sets `transactions.product_id` + `resolution_confidence` and immediately powers the user's *own* analytics ("you've bought this 6×") — low-stakes; a wrong guess is a shrug. But a line **counts toward the crowd/Price-API index only if resolved deterministically (Tier 0/1) or corroborated by a scan** — never on a bare guess. Same two-gate principle as §7.9 (personal permissive, crowd strict); protects the asset from confident-but-wrong auto-resolutions.

**MVP scope (ruling — seam + label mechanism + cheap tiers):** Tier 0 + Tier 1 + a simple `pg_trgm` Tier 2; the barcode-scan endpoint + escrowed `barcode_reward`; the `product_mappings` table; `transactions.product_code` + `resolution_confidence` + `resolution_method`.
**Later:** embedding/semantic matcher; full multi-user weighted-consensus resolution; **GTIN → Open Food Facts / GS1 enrichment** (name/brand/image from a scanned barcode); cross-chain product unification at scale; bounties for *requested* scans (§14).

## 8. GDPR & compliance (hard constraints)

- Receipts can be **Article 9 special-category** data (pharmacy → health, etc.) → treat the corpus as sensitive; use **explicit consent** as the lawful basis.
- **Self-host the model; keep all data in the EU.** No third-party processor for extraction → no DPA/Schrems exposure. If a cloud fallback is ever used, **EU-only**; **avoid US processors** (EU–US Data Privacy Framework is in acute doubt after a June 2026 US Supreme Court ruling).
- **Deletion** must cascade to the user's `receipts` + `transactions` and their receipt images in object storage **and backups**, not just the `users` row. At MVP, since crowd prices are derived from `transactions`, a deleted user's contribution leaves the aggregates (accepted; §7.3).
- **Export/portability** (Art. 20) — a full per-user export (transactions + arguably source images) should be one query.
- **Post-deletion retention** — once the de-identified `price_history` is introduced (§7.3), retaining it after account deletion is allowed **only with genuine anonymization** (real k-anonymity/aggregation), documented — most naive "anonymization" is reversible pseudonymization and would be non-compliant.

## 9. Norway specifics

- **Formats:** NOK comma decimals (`49,90`), space/period thousands, `DD.MM.YYYY` dates, MVA breakdown table (rates 25 % / 15 % / 12 %, **prices shown gross**), `pant` (deposit) and `Rabatt`/`Trumf` lines, `Totalt`/`Å betale` labels, `NO#########MVA` org-number as store id. Member/`Trumf` prices are *personal*, not shelf prices (§7.3).
- **Chain digital receipts** (Rema 1000 Æ, Coop Medlem, Trumf → Kiwi/Meny) expose perfect structured line items covering ~97 % of grocery — but via **unofficial, reverse-engineered, ToS-gray** APIs. **Opt-in Phase-3 only, never load-bearing.**
- **EHF/Peppol** is B2B/B2G e-invoicing, **not** consumer receipts — out of scope for ingestion.

## 10. Tech stack

- **Backend:** Rust (edition 2024), axum 0.8, tokio, sqlx 0.8 (Postgres + compile-time checks), argon2, jsonwebtoken, `rust-s3`, reqwest.
- **DB:** PostgreSQL (crowd prices derived from `transactions`; native partitioning + TimescaleDB later, if a retained price series is introduced).
- **Client:** React + TypeScript SPA in `web/` (Vite + Tailwind + React Router + TanStack Query + Recharts).
- **Extraction:** self-hosted Qwen3-VL via Ollama → vLLM, on an on-demand EU GPU.
- **Object storage:** S3-compatible.

## 11. Roadmap (phased)

**Launch (MVP)**
- Auth (+ `refresh_tokens`); capture/upload (photo **and manual digital-PDF upload**); durable extraction queue (on `receipts`) + self-hosted VLM; validators + confidence gate + `needs_review`; **product resolution** (Tier-0/1 deterministic + `pg_trgm` Tier-2 guess + barcode-scan-for-credit, §7.10); personal archive with filtering (by shop / item / date range) and spend analytics; credit ledger (earn on scan **+ barcode scan**); basic price search as credit-metered aggregate queries over `transactions`; export; GDPR basics (consent, export, delete-with-cascade).

**Later**
- Email/mailbox digital-receipt ingestion; **advanced product resolution** (embedding/semantic matcher, multi-user weighted-consensus, GTIN→Open Food Facts/GS1 enrichment, §7.10); chain-API opt-in import; "overpaying" comparisons; TimescaleDB for the price series; B2B paid Price API + dashboard; richer product/store identity resolution; international expansion; **crowdsourced item enrichment + demand-driven bounties + reputation/KYC (§14)**.

## 12. Open items / next steps

1. **Data model finalized (§7).** Implement it: **overwrite** the schema SQL + Rust models/handlers to match (no incremental migrations — no deployment/data yet) and do the workspace restructure.
2. Norwegian **eval set** (~50–100 receipts) to validate Qwen3-VL 4B vs 8B before locking the model.
3. Extraction worker (VLM + durable `SKIP LOCKED` queue on `receipts`).
4. Price-API contract (filters, credit metering, and the B2B-access seam).
5. Web client = React + TS SPA in `web/` (built); the Flutter `client/` was removed.
6. **Extraction cost path (§6.1):** verify the real per-receipt bill → try Gemini Flash / per-token open VLM → self-host Qwen3-VL-8B on an EU L4 → LoRA-fine-tune on the `needs_review` label queue. Merge the `debug` branch (model picker, rescan, reconciliation, concurrency fixes) once reviewed.

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
| 2026-07-09 | **No stored price table** at MVP — `transactions` already *is* the price observations; crowd price = k-anon aggregate queries over it | User: a 1:1 duplicate of the fact table is redundant |
| 2026-07-09 | De-identified retained `price_history` + `current_prices` view **deferred** to when B2B / retention / scale needs it | Consumer-first; avoid premature duplication (accept: deleted-account signal is lost until then) |
| 2026-07-09 | `purchase_at` = universal `TIMESTAMPTZ`; timezone from store address/geo → else client device tz (VPN-safe, never IP) | User decision on time/VPN |
| 2026-07-09 | `receipts` table **is** the extraction queue (`SKIP LOCKED` + retry columns); generic `jobs` table only if job types multiply | Simplest for one job type |
| 2026-07-09 | Add `refresh_tokens` for real session management | JWT access tokens alone can't rotate/revoke |
| 2026-07-09 | Layered anti-fraud (phash + signatures + arithmetic gate + idempotent ledger), stronger than a hash | Credit has spendable value; asset accrues in `transactions` |
| 2026-07-09 | Price modeled per line as **net paid** + optional **shelf price / discount** + a `price_type` tag (shelf/promo/member/coupon/net_only); index compares store-set prices | Same item has several prices at once (member/coupon/promo); model receipt-visible cases, degrade to net_only |
| 2026-07-09 | Chain loyalty rebates (1–3 % basket cashback) out of per-item price scope | Basket-level perk paid later, not a per-item price |
| 2026-07-09 | **Progressive KYC** — basic scanning/earning stays open; identity verification gates only high-value contributions / cash-out / elevated trust (provider-based, store status only, never ID docs) | Corruption-resistance without killing the funnel or holding identity data |
| 2026-07-09 | `trust_score` = earned from contributions proving true over time (corroboration); crowdsourced enrichment + bounty economy captured as vision (§14) | Hard-to-corrupt data moat; phased, post-MVP |
| 2026-07-09 | Trust engine = weighted truth discovery (Dawid–Skene-style batch EM); **KYC = high prior weight, not an oracle**; reward = value-of-information; output = value + confidence | User's KYC-weighting idea, with guardrails against KYC-as-oracle and minority-suppression |
| 2026-07-10 | MVP backend built + verified end-to-end (star schema, `ReceiptExtractor` trait mock/hosted, ingest harness, credits) | Commit 029ec07; runs on local Postgres + MinIO |
| 2026-07-10 | **Web frontend = React + TypeScript SPA** (Vite + Tailwind + TanStack Query + Recharts); Flutter `client/` **removed** | Team prefers TS; cleaner web DX. Supersedes the earlier Flutter-web choice (§5/§10). Native mobile revisited later |
| 2026-07-10 | Extraction engine = **any OpenAI-compatible vision API** (`VLM_API_KEY`). **Dev = OpenRouter**, **prod = EU-direct (Mistral)** before real users | OpenRouter = 1 key to benchmark many models; but US router → not EU-resident, so switch before real user data. Config switch, no code change |
| 2026-07-10 | Receipt→JSON **v2 schema** (nested store+address, `receipt_number`, `mva_lines`, `payment.method` no card digits, per-line `product_code`/EAN) | Richer, validatable, seeds product identity; key fields promoted to columns, rest in `raw_extraction` |
| 2026-07-11 | Extraction accuracy pass: `KORR`/void correction handling (new `correction` item type), row-alignment + per-line `qty×unit_price` prompt rules, store address persisted, currency inferred from country code, `needs_review` reconciliation (Σ line totals vs total) with reason, model self-reported `confidence`/`notes` shown in UI | Real receipts were mis-scanned (price-column misalignment, dropped items, wrong currency, hallucinated stores); reconciliation converts accuracy → review-rate |
| 2026-07-11 | Benchmarked models on 9 real receipts; **current default → `google/gemini-2.5-pro`** (8/9 reconciled vs ~5/9); `qwen-2.5-vl-72b` = best open-weight | Data over guesswork; gemini fixed the hard cases. OpenRouter is a US router → temporary; EU-direct or self-host before real users |
| 2026-07-11 | `persist_extraction` made atomic under a per-receipt `FOR UPDATE` lock; rescan endpoints atomically claim (in-flight guard, 2-min stale escape) | Adversarial review found concurrent rescans duplicated line items and an unbounded (denial-of-wallet) rescan loop |
| 2026-07-11 | **Debug tooling** (on `debug` branch): in-app model picker (`/api/debug/models`, `VLM_MODELS`), per-receipt Rescan + bulk `/api/debug/reprocess-all`, `bench_extractors`/`reprocess_all` bins, zoomable receipt viewer, confidence pill | A/B models against the reconciliation metric on live receipts; inspect/re-run without a redeploy |
| 2026-07-11 | **Cost/self-hosting roadmap set (§6.1):** Flash-first → always-on Qwen3-VL-8B/Qwen2.5-VL-7B on one L4 (~$0.012/rcpt) → LoRA fine-tune on the review-queue labels (~$0.002/rcpt). Avoid Llama-3.2-Vision (EU license carve-out) | ~$0.10/rcpt hosted won't scale to 1000/day; the reconciliation gate makes cheap/small models viable; fine-tune on our own data is the accuracy+cost moat |
| 2026-07-11 | **Receipt trust model (§7.9):** trust = **three independent statistical signals, not one score** — Fidelity (calibrated reconciliation), Authenticity (corroboration likelihood-ratio + robust-Mahalanobis novelty + dedup-as-LR), Reliability (hierarchical Bayesian truth-discovery + robust weighted aggregation); combined in **log-odds**, consumed by **two gates** (credit vs Price-API index-inclusion); enforced only where value crosses a membrane, never on the personal archive. Seams pulled to MVP: irreversible **capture-provenance** logging (`captured_at`/`capture_meta`), **escrow credit** (`ledger_settle_state`), `receipts.authenticity_score` | User: all three needed but **kept separate**, and **at least authenticity + reliability must be statistical**. The arithmetic gate proves transcription fidelity, not authenticity; the crowd price index doubles as the fraud detector (corroboration), so it self-activates as data accrues. Both MVP seams approved (capture provenance is un-backfillable; escrow is cheap now / painful later) |
| 2026-07-11 | **Product resolution (§7.10) is MVP:** receipt line → GTIN — the crowd index *depends* on it (raw strings fragment the asset; §7.3 previously assumed resolution existed but deferred it — inconsistency fixed). Cascade: Tier-0 EAN self-resolve → Tier-1 deterministic `(chain, product_code)` PLU map → Tier-2 `pg_trgm` probabilistic guess → Tier-3 **barcode-scan-for-credit** gold label (value-of-information reward, escrowed, corroborated). **Probabilistic guesses power *personal* freely but feed the *crowd* only on deterministic/corroborated resolution** (same two-gate rule as §7.9). New schema: `product_mappings` table (generalizes the old `raw_text_mappings`), `transactions.product_code`/`resolution_method`/`resolution_confidence`, `barcode_reward` reason | User flagged the gap; barcode scans are the ground-truth label flywheel (calibrate Tier-2, à la §6.1/§7.9); a scan is a claim → reuse §7.9 consensus + escrow anti-fraud |
| 2026-07-11 | **Price API privacy model (§7.3.1):** aggregate-query-only (no per-receipt endpoint); per-item independent cells with a **no-co-occurrence** rule; availability by a re-id **risk formula** (`ρ=1/n`, Poisson population-uniqueness) not a fixed K; **differential privacy** (ε budget) on price *and* volume; resolution is a shared budget traded across area/time/price; enforced at the query layer + a locked-down DB role over the same `transactions` (no schema change; receipts stay full-detail per user) | User's design; makes the price domain provably *anonymous* not just pseudonymous, keeps it commercially useful, and needs no data duplication at MVP |

## 14. Future vision — crowdsourced item enrichment & reputation

> **Post-MVP, directional.** Captured so we don't design the foundations into a corner. The MVP already accommodates it: `credit_ledger.reason` is an extensible enum, `users.trust_score` exists, and `products` + `product_mappings` (§7.10, now MVP) establish the crowdsourcing pattern.

Beyond receipts, SumPrices can become a **crowdsourced product-knowledge graph** — users earn credit not only for scanning but for *enriching* items, can *request* information, and a reputation system makes the data hard to corrupt.

- **Contributions (earn credit by enriching an item):** ingredients-list photo, a general product photo, weight / dimensions, a manual (furniture / Lego), etc. → a flexible `item_contributions` table (`product_id`, `attribute_type`, value / `asset_key`, `contributed_by`, `confidence`, verification status). Typed, so new attribute types are config, not migrations.
- **Requests & demand-driven bounties:** users *request* a missing attribute (`info_requests`); **reward scales with demand** (more distinct requesters for the same attribute → higher credit) **and difficulty** (a photo is easy; provenance is hard). Fulfilment credits the contributor via a new `credit_ledger.reason` (`contribution_reward` / `bounty_reward`).
- **Trust & truth-over-time (weighted truth discovery):** model it as **Dawid–Skene-style weighted consensus** — a periodic batch EM job over all contributions jointly infers each claim's most-likely value **+ a confidence** *and* each contributor's reliability. `trust_score` = a contributor's estimated reliability, earned from **how often their past contributions match the inferred truth** as data accumulates. Output is **value + confidence, never a binary** (the confidence is itself a B2B selling point). *Research-grade (Sybil/collusion resistance, convergence) — phase it: simple corroboration first, then a proper statistical model. Frameworks: Dawid–Skene + Bayesian variants, truth-discovery (TruthFinder / CRH / CATD), IRT ability/difficulty models, Beta-reputation / EigenTrust.*
- **KYC as a weight, not an oracle:** KYC gives a user a **high prior weight** in the consensus and unlocks a higher earning tier (+ a signup bonus) — but a KYC user is **still updated by their own track record** and can be wrong. **Guardrails:** require *multiple independent* high-trust confirmations before a claim is "near-true"; **cap any single actor's weight**; detect collusion (correlated voting clusters); **don't hard-punish disagreement** (the minority is sometimes right — let truth shift over time). Non-KYC users' reliability is calibrated from agreement with the trust-weighted consensus. Progressive: **basic scanning/earning stays open** (KYC would kill the funnel); KYC gates only high-value contributions / cash-out / elevated trust. Provider-based (Vipps / BankID in NO; Stripe Identity abroad), storing only `kyc_status` + a reference — **never** ID documents.
- **Reward = value of information:** credit ≈ **demand × difficulty × info-gain × contributor-weight** — pay most for the validation that most reduces a claim's uncertainty (e.g. a scarce KYC confirmation of a claim many anonymous users asserted — the user's original instinct, generalized). Objective attributes (weight, ingredients-as-printed, barcode) converge well; subjective / hard-to-verify ones (country-of-origin / provenance) don't — keep those low-confidence or deferred.
- **Anti-gaming:** fake requests, collusion rings, and self-fulfilment are resisted by KYC + `trust_score` weighting + rate limits + the same idempotent, auditable `credit_ledger`.

**Future tables:** `item_contributions`, `contribution_types`, `info_requests` (+ demand count), `contribution_verifications`; KYC fields on `users` (`kyc_status`, `kyc_ref`); new `credit_ledger.reason` values.
