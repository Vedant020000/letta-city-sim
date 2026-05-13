INSERT INTO shops (id, name, location_prefix, owner_id, shopkeeper_job_id, balance_cents)
VALUES
  ('harvey_oak', 'Harvey Oak Supermart', 'harvey_oak', 'rosie_kim', 'shopkeeper', 50000),
  ('hobbs_cafe', 'Hobbs Cafe', 'hobbs_cafe', 'isabella_rodriguez', 'cafe_owner', 30000)
ON CONFLICT (id) DO UPDATE
SET name = EXCLUDED.name,
    location_prefix = EXCLUDED.location_prefix,
    owner_id = EXCLUDED.owner_id,
    shopkeeper_job_id = EXCLUDED.shopkeeper_job_id,
    balance_cents = EXCLUDED.balance_cents,
    is_active = TRUE,
    updated_at = NOW();
