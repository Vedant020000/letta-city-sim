-- Job system: employment tracking, wages, and lifecycle

-- Add employment fields to jobs
ALTER TABLE jobs ADD COLUMN employer_id TEXT REFERENCES agents(id);
ALTER TABLE jobs ADD COLUMN wage_cents BIGINT;
ALTER TABLE jobs ADD COLUMN pay_period_minutes INTEGER NOT NULL DEFAULT 60;
ALTER TABLE jobs ADD COLUMN is_city_job BOOLEAN NOT NULL DEFAULT FALSE;

-- Add lifecycle tracking to agent_jobs
ALTER TABLE agent_jobs ADD COLUMN status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('pending', 'active', 'resigned', 'fired', 'on_leave'));
ALTER TABLE agent_jobs ADD COLUMN hired_at TIMESTAMPTZ NOT NULL DEFAULT NOW();
ALTER TABLE agent_jobs ADD COLUMN last_paid_at TIMESTAMPTZ;
ALTER TABLE agent_jobs ADD COLUMN resigned_at TIMESTAMPTZ;

-- Index for finding employees by employer
CREATE INDEX idx_jobs_employer_id ON jobs(employer_id);
-- Index for finding active assignments
CREATE INDEX idx_agent_jobs_status ON agent_jobs(status);
