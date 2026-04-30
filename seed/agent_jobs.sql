INSERT INTO agent_jobs (agent_id, job_id, is_primary, notes)
VALUES
  ('eddy_lin', 'music_student', TRUE, 'Starter assignment seeded from the existing occupation/persona.'),
  ('isabella_rodriguez', 'cafe_owner', TRUE, 'Starter assignment seeded from the existing occupation/persona.'),
  ('klaus_mueller', 'professor', TRUE, 'Starter assignment seeded from the existing occupation/persona.'),
  ('maria_lopez', 'artist', TRUE, 'Starter assignment seeded from the existing occupation/persona.'),
  ('sam_moore', 'shop_assistant', TRUE, 'Starter assignment seeded from the existing occupation/persona.'),
  ('abigail_chen', 'student', TRUE, 'Starter assignment seeded from the existing occupation/persona.')
ON CONFLICT (agent_id, job_id) DO UPDATE
SET is_primary = EXCLUDED.is_primary,
    notes = EXCLUDED.notes,
    updated_at = NOW();
