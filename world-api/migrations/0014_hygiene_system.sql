-- Hygiene and appearance system

ALTER TABLE agents ADD COLUMN hygiene_level SMALLINT NOT NULL DEFAULT 100;
ALTER TABLE agents ADD COLUMN appearance_level SMALLINT NOT NULL DEFAULT 100;
