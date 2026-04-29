CREATE TABLE agent_intentions (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    summary TEXT NOT NULL,
    reason TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    expected_location_id TEXT REFERENCES locations(id) ON DELETE SET NULL,
    expected_action TEXT,
    outcome TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    CHECK (status IN ('active','completed','failed','abandoned')),
    CHECK (length(btrim(summary)) > 0),
    CHECK (length(btrim(reason)) > 0)
);

CREATE UNIQUE INDEX idx_agent_intentions_one_active
    ON agent_intentions(agent_id)
    WHERE status = 'active';

CREATE INDEX idx_agent_intentions_agent_created
    ON agent_intentions(agent_id, created_at DESC);
