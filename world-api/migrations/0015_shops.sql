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

-- Seed Harvey Oak Supermart
INSERT INTO shops (id, name, location_prefix, owner_id, shopkeeper_job_id, balance_cents)
VALUES ('harvey_oak', 'Harvey Oak Supermart', 'harvey_oak', 'rosie_kim', 'shopkeeper', 50000);

-- Seed Hobbs Cafe
INSERT INTO shops (id, name, location_prefix, owner_id, shopkeeper_job_id, balance_cents)
VALUES ('hobbs_cafe', 'Hobbs Cafe', 'hobbs_cafe', 'isabella_morgan', 'cafe_owner', 30000);
