-- Shops foundation: make shops a first-class concept

CREATE TABLE shops (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    location_prefix TEXT NOT NULL UNIQUE,
    owner_id TEXT REFERENCES agents(id),
    shopkeeper_job_id TEXT REFERENCES jobs(id),
    balance_cents BIGINT NOT NULL DEFAULT 0,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
