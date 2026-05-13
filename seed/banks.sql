INSERT INTO banks (
  id,
  name,
  location_id,
  balance_cents,
  deposit_rate_daily,
  loan_rate_daily,
  reserve_ratio,
  updated_by
)
VALUES
  (
    'smallville_bank',
    'Smallville Bank',
    'smallville_bank_lobby',
    100000,
    0.0005,
    0.0020,
    0.10,
    'nora_patel'
  )
ON CONFLICT (id) DO UPDATE
SET name = EXCLUDED.name,
    location_id = EXCLUDED.location_id,
    reserve_ratio = EXCLUDED.reserve_ratio,
    updated_at = NOW();
