-- Construction sector: companies and projects

CREATE TABLE construction_companies (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    progress_per_sim_hour INT NOT NULL DEFAULT 10,
    hiring_fee_cents BIGINT NOT NULL DEFAULT 1000,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE construction_projects (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    location_name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'planning' CHECK (status IN ('planning', 'funding', 'building', 'complete')),
    cost_cents BIGINT NOT NULL DEFAULT 5000,
    funded_cents BIGINT NOT NULL DEFAULT 0,
    progress INT NOT NULL DEFAULT 0 CHECK (progress >= 0 AND progress <= 100),
    company_id TEXT REFERENCES construction_companies(id),
    last_progress_tick TIMESTAMPTZ,
    location_id TEXT REFERENCES locations(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX idx_construction_projects_agent ON construction_projects(agent_id, status);
CREATE INDEX idx_construction_projects_status ON construction_projects(status);
