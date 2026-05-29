ALTER TABLE agents ADD COLUMN travel_destination_id TEXT REFERENCES locations(id);
ALTER TABLE agents ADD COLUMN travel_started_at TIMESTAMPTZ;
ALTER TABLE agents ADD COLUMN travel_arrives_at TIMESTAMPTZ;
ALTER TABLE agents ADD COLUMN travel_path JSONB;
ALTER TABLE agents ADD COLUMN travel_total_secs INTEGER;
ALTER TABLE agents ADD COLUMN travel_from_location_id TEXT REFERENCES locations(id);

ALTER TABLE agents DROP CONSTRAINT IF EXISTS agents_state_check;
ALTER TABLE agents ADD CONSTRAINT agents_state_check
    CHECK (state IN ('idle','walking','working','conversing','sleeping','paused','traveling'));

CREATE INDEX idx_agents_travel_arrives ON agents(travel_arrives_at)
    WHERE travel_arrives_at IS NOT NULL;
