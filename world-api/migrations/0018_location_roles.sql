-- Location roles, capacity, and ownership

ALTER TABLE locations
    ADD COLUMN kind TEXT NOT NULL DEFAULT 'public',
    ADD COLUMN capacity INTEGER,
    ADD COLUMN metadata JSONB NOT NULL DEFAULT '{}'::JSONB;

-- Classify existing seed locations
UPDATE locations SET kind = 'home' WHERE id LIKE 'lin_%';
UPDATE locations SET kind = 'business' WHERE id LIKE 'hobbs_cafe_%' OR id LIKE 'harvey_oak_%';
UPDATE locations SET kind = 'workplace' WHERE id LIKE 'oak_%' OR id LIKE 'smallville_bank_%';
UPDATE locations SET kind = 'public' WHERE id LIKE 'ville_park_%' OR id = 'notice_board' OR id LIKE 'miller_community_garden';
UPDATE locations SET kind = 'civic' WHERE id LIKE 'townhall_%' OR id LIKE 'riverside_clinic_%';
UPDATE locations SET kind = 'public' WHERE id LIKE 'smallville_library_%';

CREATE TABLE location_roles (
    location_id TEXT NOT NULL REFERENCES locations(id) ON DELETE CASCADE,
    agent_id TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('owner', 'resident', 'tenant', 'worker', 'manager')),
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (location_id, agent_id, role)
);

CREATE INDEX idx_location_roles_location ON location_roles(location_id);
CREATE INDEX idx_location_roles_agent ON location_roles(agent_id);
CREATE INDEX idx_location_roles_role ON location_roles(role);
