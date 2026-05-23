INSERT INTO construction_companies (id, name, progress_per_sim_hour, hiring_fee_cents, is_active)
VALUES (
    'smallville_construction',
    'Smallville Construction Co.',
    10,
    1000,
    TRUE
)
ON CONFLICT (id) DO UPDATE
SET name = EXCLUDED.name,
    progress_per_sim_hour = EXCLUDED.progress_per_sim_hour,
    hiring_fee_cents = EXCLUDED.hiring_fee_cents,
    is_active = EXCLUDED.is_active;
