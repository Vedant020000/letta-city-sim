INSERT INTO locations (id, name, description, map_x, map_y)
VALUES
  ('lin_bedroom', 'Eddy''s Bedroom', 'A small bedroom with a single bed and a desk with sheet music on it.', 128, 96),
  ('lin_kitchen', 'Lin Family Kitchen', 'A modest kitchen with a stove, refrigerator, and a small dining table.', 128, 160),
  ('lin_living_room', 'Lin Family Living Room', 'A cosy living room with a piano, a bookshelf, and a worn-out sofa.', 224, 128),
  ('hobbs_cafe_counter', 'Hobbs Cafe Counter', 'The front counter of Hobbs Cafe. Isabella usually works here in the mornings.', 480, 256),
  ('hobbs_cafe_seating', 'Hobbs Cafe Seating', 'A warmly lit seating area with four small tables. Often busy in the morning.', 576, 256),
  ('hobbs_cafe_kitchen', 'Hobbs Cafe Kitchen', 'The small kitchen behind the counter where coffee and pastries are made.', 480, 192),
  ('ville_park_east', 'Ville Park (East Bench)', 'A quiet park bench near the east fountain. Good for people-watching.', 768, 384),
  ('ville_park_west', 'Ville Park (West Bench)', 'The west side of Ville Park, near the notice board.', 672, 384),
  ('notice_board', 'The Notice Board', 'A community notice board at the edge of Ville Park. Anyone can post here.', 704, 352),
  ('harvey_oak_floor', 'Harvey Oak Supply (Shop Floor)', 'A small general supply store. Sam works at the counter here.', 320, 480),
  ('oak_classroom_a', 'Oak Hill College - Classroom A', 'A lecture hall where Klaus teaches. Rows of chairs face a chalkboard.', 896, 128),
  ('oak_staff_office', 'Oak Hill College - Staff Office', 'A cluttered office with stacks of papers and two desks.', 960, 128)
ON CONFLICT (id) DO UPDATE
SET name = EXCLUDED.name,
    description = EXCLUDED.description,
    map_x = EXCLUDED.map_x,
    map_y = EXCLUDED.map_y;