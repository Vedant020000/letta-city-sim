-- Bank sector: deposits, loans, interest rates, and ledger

CREATE TABLE banks (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    location_prefix TEXT NOT NULL UNIQUE,
    balance_cents BIGINT NOT NULL DEFAULT 0,
    banker_job_id TEXT REFERENCES jobs(id),
    deposit_rate_daily DOUBLE PRECISION NOT NULL DEFAULT 0.0005 CHECK (deposit_rate_daily >= 0),
    loan_rate_daily DOUBLE PRECISION NOT NULL DEFAULT 0.0020 CHECK (loan_rate_daily >= 0),
    reserve_ratio DOUBLE PRECISION NOT NULL DEFAULT 0.10 CHECK (reserve_ratio >= 0 AND reserve_ratio <= 1),
    opens_at SMALLINT NOT NULL DEFAULT 9,
    closes_at SMALLINT NOT NULL DEFAULT 17,
    updated_by TEXT REFERENCES agents(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (loan_rate_daily >= deposit_rate_daily)
);

CREATE TABLE bank_accounts (
    bank_id TEXT NOT NULL REFERENCES banks(id) ON DELETE CASCADE,
    agent_id TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    balance_cents BIGINT NOT NULL DEFAULT 0 CHECK (balance_cents >= 0),
    last_accrued_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (bank_id, agent_id)
);

CREATE TABLE bank_loans (
    id TEXT PRIMARY KEY,
    bank_id TEXT NOT NULL REFERENCES banks(id) ON DELETE CASCADE,
    agent_id TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    principal_cents BIGINT NOT NULL CHECK (principal_cents > 0),
    outstanding_cents BIGINT NOT NULL CHECK (outstanding_cents >= 0),
    daily_rate DOUBLE PRECISION NOT NULL CHECK (daily_rate >= 0),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'paid', 'defaulted', 'forgiven')),
    last_accrued_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    closed_at TIMESTAMPTZ
);

CREATE INDEX idx_bank_loans_agent_status
    ON bank_loans(agent_id, status);

CREATE INDEX idx_bank_loans_bank_status
    ON bank_loans(bank_id, status);

CREATE TABLE bank_ledger_entries (
    id BIGSERIAL PRIMARY KEY,
    bank_id TEXT NOT NULL REFERENCES banks(id) ON DELETE CASCADE,
    agent_id TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    loan_id TEXT REFERENCES bank_loans(id) ON DELETE SET NULL,
    entry_type TEXT NOT NULL CHECK (entry_type IN (
        'deposit', 'withdrawal', 'deposit_interest', 'loan_disbursement',
        'loan_interest', 'loan_repayment', 'rate_change'
    )),
    amount_cents BIGINT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_bank_ledger_bank_created
    ON bank_ledger_entries(bank_id, created_at DESC);

CREATE INDEX idx_bank_ledger_agent_created
    ON bank_ledger_entries(agent_id, created_at DESC);
