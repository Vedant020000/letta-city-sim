-- Schema initialization for letta-city-sim

CREATE TABLE locations (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    map_x INTEGER NOT NULL,
    map_y INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE location_adjacency (
    from_id TEXT REFERENCES locations(id) ON DELETE CASCADE,
    to_id TEXT REFERENCES locations(id) ON DELETE CASCADE,
    travel_secs INTEGER NOT NULL,
    PRIMARY KEY (from_id, to_id)
);

CREATE TABLE agents (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    occupation TEXT NOT NULL,
    persona_summary TEXT,
    current_location_id TEXT NOT NULL REFERENCES locations(id),
    state TEXT NOT NULL DEFAULT 'idle',
    current_activity TEXT,
    activity_started_at TIMESTAMPTZ,
    state_updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    is_npc BOOLEAN NOT NULL DEFAULT TRUE,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    letta_agent_id TEXT NOT NULL,
    letta_message_endpoint TEXT,
    last_wake_reason TEXT,
    last_seen_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    home_location_id TEXT REFERENCES locations(id),
    CHECK (state IN ('idle','walking','working','conversing','sleeping','paused'))
);

CREATE INDEX idx_agents_location ON agents(current_location_id);
CREATE INDEX idx_agents_state ON agents(state);
CREATE INDEX idx_agents_active ON agents(is_active, state);

CREATE TABLE world_objects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    location_id TEXT REFERENCES locations(id) ON DELETE SET NULL,
    state JSONB NOT NULL DEFAULT '{}'::JSONB,
    actions TEXT[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE inventory_items (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    held_by TEXT REFERENCES agents(id),
    location_id TEXT REFERENCES locations(id),
    state JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT held_xor_located CHECK ((held_by IS NULL) <> (location_id IS NULL))
);

CREATE INDEX idx_inventory_held ON inventory_items(held_by);
CREATE INDEX idx_inventory_location ON inventory_items(location_id);

CREATE TABLE conversations (
    id TEXT PRIMARY KEY,
    location_id TEXT REFERENCES locations(id),
    topic TEXT,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ended_at TIMESTAMPTZ
);

CREATE TABLE conversation_participants (
    conversation_id TEXT REFERENCES conversations(id) ON DELETE CASCADE,
    agent_id TEXT REFERENCES agents(id) ON DELETE CASCADE,
    joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    left_at TIMESTAMPTZ,
    PRIMARY KEY (conversation_id, agent_id)
);

CREATE TABLE conversation_messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT REFERENCES conversations(id) ON DELETE CASCADE,
    agent_id TEXT REFERENCES agents(id) ON DELETE SET NULL,
    content TEXT NOT NULL,
    sent_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_conv_active ON conversations(location_id) WHERE ended_at IS NULL;
CREATE INDEX idx_conv_messages_conversation ON conversation_messages(conversation_id);
CREATE INDEX idx_conv_messages_agent ON conversation_messages(agent_id);

CREATE TABLE events (
    id BIGSERIAL PRIMARY KEY,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    type TEXT NOT NULL,
    actor_id TEXT REFERENCES agents(id),
    location_id TEXT REFERENCES locations(id),
    description TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB
);

CREATE INDEX idx_events_location ON events(location_id, occurred_at DESC);
CREATE INDEX idx_events_actor ON events(actor_id, occurred_at DESC);

CREATE TABLE simulation_state (
    id TEXT PRIMARY KEY,
    key TEXT NOT NULL,
    value JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
