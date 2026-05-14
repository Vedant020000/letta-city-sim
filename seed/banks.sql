INSERT INTO banks (
  id,
  name,
  location_prefix,
  banker_job_id,
  balance_cents,
  deposit_rate_daily,
  loan_rate_daily,
  reserve_ratio,
  opens_at,
  closes_at,
  updated_by
)
VALUES
  (
    'smallville_bank',
    'Smallville Bank',
    'smallville_bank',
    'banker',
    100000,
    0.0005,
    0.0020,
    0.10,
    9,
    17,
    'nora_patel'
  )
ON CONFLICT (id) DO UPDATE
SET name = EXCLUDED.name,
    location_prefix = EXCLUDED.location_prefix,
    banker_job_id = EXCLUDED.banker_job_id,
    reserve_ratio = EXCLUDED.reserve_ratio,
    opens_at = EXCLUDED.opens_at,
    closes_at = EXCLUDED.closes_at,
    updated_at = NOW();
