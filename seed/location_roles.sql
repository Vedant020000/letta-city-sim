INSERT INTO location_roles (location_id, agent_id, role)
VALUES
  -- Eddy Lin lives at the Lin family home
  ('lin_bedroom', 'eddy_lin', 'resident'),
  ('lin_kitchen', 'eddy_lin', 'resident'),
  ('lin_living_room', 'eddy_lin', 'resident'),

  -- Isabella Rodriguez owns and lives at Hobbs Cafe
  ('hobbs_cafe_seating', 'isabella_rodriguez', 'resident'),
  ('hobbs_cafe_counter', 'isabella_rodriguez', 'owner'),
  ('hobbs_cafe_counter', 'isabella_rodriguez', 'worker'),

  -- Rosie Kim owns and lives at Harvey Oak Supermart
  ('harvey_oak_checkout', 'rosie_kim', 'resident'),
  ('harvey_oak_checkout', 'rosie_kim', 'owner'),
  ('harvey_oak_checkout', 'rosie_kim', 'worker'),

  -- Sam Moore lives and works at Harvey Oak
  ('harvey_oak_aisle', 'sam_moore', 'resident'),
  ('harvey_oak_aisle', 'sam_moore', 'worker'),

  -- Klaus Mueller lives and works at Town Hall, also teaches at Oak Hill
  ('townhall_mayor_office', 'klaus_mueller', 'resident'),
  ('townhall_mayor_office', 'klaus_mueller', 'worker'),
  ('oak_classroom_a', 'klaus_mueller', 'worker'),

  -- Maria Lopez lives at Ville Park
  ('ville_park_west', 'maria_lopez', 'resident'),

  -- Abigail Chen lives at Oak Hill College
  ('oak_classroom_a', 'abigail_chen', 'resident'),

  -- Nora Patel lives and works at the bank
  ('smallville_bank_office', 'nora_patel', 'resident'),
  ('smallville_bank_office', 'nora_patel', 'worker')
ON CONFLICT (location_id, agent_id, role) DO NOTHING;
