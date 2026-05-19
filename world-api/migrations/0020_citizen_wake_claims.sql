ALTER TABLE citizen_wakes
    ADD COLUMN claimed_at TIMESTAMPTZ,
    ADD COLUMN claim_expires_at TIMESTAMPTZ,
    ADD COLUMN claimed_by TEXT;

CREATE INDEX idx_citizen_wakes_agent_claimable
    ON citizen_wakes(agent_id, status, seq)
    WHERE status = 'open' AND closed_at IS NULL;
