WITH seed_assignments(agent_id, job_id, is_primary) AS (
  VALUES
    ('eddy_lin', 'music_student', TRUE),
    ('isabella_rodriguez', 'cafe_owner', TRUE),
    ('klaus_mueller', 'mayor', TRUE),
    ('maria_lopez', 'artist', TRUE),
    ('sam_moore', 'shop_assistant', TRUE),
    ('abigail_chen', 'student', TRUE),
    ('rosie_kim', 'shopkeeper', TRUE),
    ('nora_patel', 'banker', TRUE)
)
UPDATE agent_jobs existing
SET is_primary = FALSE,
    updated_at = NOW()
FROM seed_assignments seed
WHERE existing.agent_id = seed.agent_id
  AND seed.is_primary = TRUE
  AND existing.is_primary = TRUE
  AND existing.job_id <> seed.job_id;

INSERT INTO agent_jobs (agent_id, job_id, is_primary, notes, status)
VALUES
  ('eddy_lin', 'music_student', TRUE, 'Starter assignment seeded from the existing occupation/persona.', 'active'),
  ('isabella_rodriguez', 'cafe_owner', TRUE, 'Starter assignment seeded from the existing occupation/persona.', 'active'),
  ('klaus_mueller', 'mayor', TRUE, 'Appointed as the first town mayor.', 'active'),
  ('maria_lopez', 'artist', TRUE, 'Starter assignment seeded from the existing occupation/persona.', 'active'),
  ('sam_moore', 'shop_assistant', TRUE, 'Starter assignment seeded from the existing occupation/persona.', 'active'),
  ('abigail_chen', 'student', TRUE, 'Starter assignment seeded from the existing occupation/persona.', 'active'),
  ('rosie_kim', 'shopkeeper', TRUE, 'Starter assignment seeded from the existing occupation/persona.', 'active'),
  ('nora_patel', 'banker', TRUE, 'Starter assignment seeded for the bank sector.', 'active')
ON CONFLICT (agent_id, job_id) DO UPDATE
SET is_primary = EXCLUDED.is_primary,
    notes = EXCLUDED.notes,
    status = EXCLUDED.status,
    updated_at = NOW();
