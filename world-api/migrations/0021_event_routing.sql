-- Event routing: add importance and visibility to events

ALTER TABLE events ADD COLUMN IF NOT EXISTS importance SMALLINT NOT NULL DEFAULT 2;
ALTER TABLE events ADD COLUMN IF NOT EXISTS visibility TEXT NOT NULL DEFAULT 'location'
    CHECK (visibility IN ('public', 'location', 'actor', 'target'));

CREATE INDEX IF NOT EXISTS idx_events_importance ON events(importance, occurred_at DESC);
