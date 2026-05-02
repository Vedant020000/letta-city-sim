CREATE TABLE citizen_runtime_state (
    agent_id TEXT PRIMARY KEY REFERENCES agents(id) ON DELETE CASCADE,
    last_seq BIGINT NOT NULL DEFAULT 0,
    pending_dropped_overflow_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE citizen_wakes (
    event_id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    seq BIGINT NOT NULL,
    wake_type TEXT NOT NULL,
    world_time TIMESTAMPTZ NOT NULL,
    wall_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    agent_snapshot JSONB NOT NULL,
    trigger_payload JSONB NOT NULL,
    prompt_narrative TEXT NOT NULL,
    prompt_structured JSONB,
    tools JSONB NOT NULL DEFAULT '[]'::JSONB,
    wake_token_expires_at TIMESTAMPTZ NOT NULL,
    expects_response BOOLEAN NOT NULL DEFAULT TRUE,
    dropped_for_overflow_count INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL,
    abort_reason TEXT,
    opened_at TIMESTAMPTZ,
    closed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT citizen_wakes_status_check CHECK (
        status IN ('queued', 'open', 'done', 'aborted', 'expired', 'dropped')
    ),
    CONSTRAINT citizen_wakes_agent_seq_unique UNIQUE (agent_id, seq)
);

CREATE INDEX idx_citizen_wakes_agent_status_seq
    ON citizen_wakes(agent_id, status, seq);

CREATE TABLE citizen_action_receipts (
    id BIGSERIAL PRIMARY KEY,
    agent_id TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    client_event_id TEXT NOT NULL,
    wake_event_id TEXT NOT NULL REFERENCES citizen_wakes(event_id) ON DELETE CASCADE,
    response_status INTEGER NOT NULL,
    response_body JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT citizen_action_receipts_agent_event_unique UNIQUE (agent_id, client_event_id)
);

CREATE INDEX idx_citizen_action_receipts_agent_wake
    ON citizen_action_receipts(agent_id, wake_event_id);
