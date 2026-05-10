-- Add pricing support for shop items

ALTER TABLE inventory_items
    ADD COLUMN price_cents BIGINT;

-- Index for finding priced items at a location (for buy_item manifest conditional)
CREATE INDEX idx_inventory_priced ON inventory_items(location_id) WHERE price_cents IS NOT NULL;
