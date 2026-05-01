CREATE TABLE agent_tokens (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    label TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ
);

CREATE INDEX idx_agent_tokens_agent
    ON agent_tokens(agent_id, created_at DESC);

CREATE INDEX idx_agent_tokens_active_hash
    ON agent_tokens(token_hash)
    WHERE revoked_at IS NULL;
