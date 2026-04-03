INSERT INTO location_adjacency (from_id, to_id, travel_secs)
VALUES
  ('lin_bedroom', 'lin_kitchen', 15),
  ('lin_kitchen', 'lin_bedroom', 15),
  ('lin_kitchen', 'lin_living_room', 10),
  ('lin_living_room', 'lin_kitchen', 10),
  ('lin_living_room', 'hobbs_cafe_seating', 120),
  ('hobbs_cafe_seating', 'lin_living_room', 120),
  ('hobbs_cafe_counter', 'hobbs_cafe_seating', 10),
  ('hobbs_cafe_seating', 'hobbs_cafe_counter', 10),
  ('hobbs_cafe_counter', 'hobbs_cafe_kitchen', 8),
  ('hobbs_cafe_kitchen', 'hobbs_cafe_counter', 8),
  ('hobbs_cafe_seating', 'ville_park_east', 60),
  ('ville_park_east', 'hobbs_cafe_seating', 60),
  ('ville_park_east', 'ville_park_west', 20),
  ('ville_park_west', 'ville_park_east', 20),
  ('ville_park_west', 'notice_board', 5),
  ('notice_board', 'ville_park_west', 5),
  ('ville_park_west', 'harvey_oak_floor', 90),
  ('harvey_oak_floor', 'ville_park_west', 90),
  ('oak_classroom_a', 'oak_staff_office', 30),
  ('oak_staff_office', 'oak_classroom_a', 30),
  ('oak_staff_office', 'hobbs_cafe_seating', 180),
  ('hobbs_cafe_seating', 'oak_staff_office', 180)
ON CONFLICT (from_id, to_id) DO UPDATE
SET travel_secs = EXCLUDED.travel_secs;