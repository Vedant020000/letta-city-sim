-- Economy transactions log + money transfer support

CREATE TABLE economy_transactions (
    id BIGSERIAL PRIMARY KEY,
    from_agent_id TEXT REFERENCES agents(id),
    to_agent_id TEXT REFERENCES agents(id),
    amount_cents BIGINT NOT NULL CHECK (amount_cents > 0),
    reason TEXT,
    transaction_type TEXT NOT NULL CHECK (transaction_type IN ('payment', 'money_request', 'salary', 'manual')),
    status TEXT NOT NULL DEFAULT 'completed' CHECK (status IN ('pending', 'completed', 'rejected', 'cancelled')),
    location_id TEXT REFERENCES locations(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ
);

CREATE INDEX idx_econ_tx_from ON economy_transactions(from_agent_id);
CREATE INDEX idx_econ_tx_to ON economy_transactions(to_agent_id);
CREATE INDEX idx_econ_tx_status ON economy_transactions(status);
