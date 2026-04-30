CREATE TABLE jobs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    summary TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (kind IN ('town','meta')),
    CHECK (length(btrim(id)) > 0),
    CHECK (length(btrim(name)) > 0),
    CHECK (length(btrim(summary)) > 0)
);

CREATE TABLE agent_jobs (
    agent_id TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    job_id TEXT NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    is_primary BOOLEAN NOT NULL DEFAULT FALSE,
    notes TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (agent_id, job_id)
);

CREATE INDEX idx_jobs_kind_name ON jobs(kind, name);
CREATE INDEX idx_agent_jobs_job_id ON agent_jobs(job_id);
CREATE UNIQUE INDEX idx_agent_jobs_one_primary
    ON agent_jobs(agent_id)
    WHERE is_primary = TRUE;
