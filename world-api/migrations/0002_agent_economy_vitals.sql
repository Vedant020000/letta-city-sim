-- Agent economy + vitals scaffolding
ALTER TABLE agents
    ADD COLUMN balance_cents BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN last_income_cents BIGINT,
    ADD COLUMN last_income_reason TEXT,
    ADD COLUMN last_income_at TIMESTAMPTZ,
    ADD COLUMN last_expense_cents BIGINT,
    ADD COLUMN last_expense_reason TEXT,
    ADD COLUMN last_expense_at TIMESTAMPTZ,
    ADD COLUMN food_level SMALLINT NOT NULL DEFAULT 100 CHECK (food_level BETWEEN 0 AND 100),
    ADD COLUMN water_level SMALLINT NOT NULL DEFAULT 100 CHECK (water_level BETWEEN 0 AND 100),
    ADD COLUMN stamina_level SMALLINT NOT NULL DEFAULT 80 CHECK (stamina_level BETWEEN 0 AND 100),
    ADD COLUMN sleep_level SMALLINT NOT NULL DEFAULT 100 CHECK (sleep_level BETWEEN 0 AND 100),
    ADD COLUMN last_vitals_update TIMESTAMPTZ NOT NULL DEFAULT NOW();
