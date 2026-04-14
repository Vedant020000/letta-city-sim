-- Add stackable consumables support to inventory_items

ALTER TABLE inventory_items
    ADD COLUMN quantity SMALLINT NOT NULL DEFAULT 1,
    ADD COLUMN consumable_type TEXT,
    ADD COLUMN vital_value SMALLINT;

CREATE INDEX idx_inventory_consumable ON inventory_items(consumable_type) WHERE consumable_type IS NOT NULL;
