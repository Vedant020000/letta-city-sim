-- Agent registration applications

CREATE TABLE agent_applications (
    id TEXT PRIMARY KEY,
    requested_agent_id TEXT,
    requested_name TEXT NOT NULL,
    occupation TEXT NOT NULL,
    statement TEXT NOT NULL,
    agent_description TEXT,
    callback_url TEXT,
    external_agent_ref TEXT,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'rejected', 'withdrawn')),
    review_note TEXT,
    approved_agent_id TEXT REFERENCES agents(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    reviewed_at TIMESTAMPTZ
);

CREATE INDEX idx_agent_applications_status ON agent_applications(status);
CREATE INDEX idx_agent_applications_created ON agent_applications(created_at DESC);
