INSERT INTO world_objects (id, name, location_id, state, actions)
VALUES
  ('bed_lin_bedroom', 'Eddy''s Bed', 'lin_bedroom', '{"occupied_by": null}', ARRAY['sleep']),
  ('stove_lin_kitchen', 'Lin Kitchen Stove', 'lin_kitchen', '{"on": false}', ARRAY['turn_on', 'turn_off', 'cook']),
  ('piano_lin_living', 'Living Room Piano', 'lin_living_room', '{"in_use": false}', ARRAY['play', 'practice']),
  ('coffee_machine_hobbs', 'Hobbs Coffee Machine', 'hobbs_cafe_counter', '{"on": true}', ARRAY['brew', 'clean']),
  ('notice_board_main', 'Notice Board', 'notice_board', '{"posts": []}', ARRAY['post', 'read']),
  ('bench_park_east', 'East Park Bench', 'ville_park_east', '{}', ARRAY['sit']),
  ('bench_park_west', 'West Park Bench', 'ville_park_west', '{}', ARRAY['sit']),
  ('shop_counter_harvey', 'Harvey Oak Counter', 'harvey_oak_floor', '{}', ARRAY['buy', 'sell'])
ON CONFLICT (id) DO UPDATE
SET name = EXCLUDED.name,
    location_id = EXCLUDED.location_id,
    state = EXCLUDED.state,
    actions = EXCLUDED.actions;

INSERT INTO inventory_items (id, name, held_by, location_id, state)
VALUES
  ('coffee_beans_001', 'Coffee Beans', NULL, 'hobbs_cafe_kitchen', '{}'),
  ('sheet_music_001', 'Sheet Music', NULL, 'lin_bedroom', '{}'),
  ('paint_brush_001', 'Paint Brush', NULL, 'ville_park_west', '{}'),
  ('notebook_001', 'Lecture Notebook', NULL, 'oak_classroom_a', '{}'),
  ('apple_001', 'Apple', NULL, 'harvey_oak_floor', '{}')
ON CONFLICT (id) DO UPDATE
SET name = EXCLUDED.name,
    held_by = EXCLUDED.held_by,
    location_id = EXCLUDED.location_id,
    state = EXCLUDED.state;
