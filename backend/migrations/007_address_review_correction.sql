-- Store address denormalized onto the receipt (the VLM already reads it; we just
-- weren't persisting it), a human-readable `review_reason`, and a `correction`
-- item type for cashier voids/reversals (e.g. Norwegian "KORR." lines).

ALTER TABLE receipts
    ADD COLUMN store_address      TEXT,
    ADD COLUMN store_city         TEXT,
    ADD COLUMN store_postal_code  TEXT,
    ADD COLUMN store_country_code TEXT,
    ADD COLUMN review_reason      TEXT;

-- PG 12+ allows ALTER TYPE ... ADD VALUE inside the migration transaction as long
-- as the new value isn't used in that same transaction (it isn't — only at runtime).
ALTER TYPE item_type ADD VALUE IF NOT EXISTS 'correction';
