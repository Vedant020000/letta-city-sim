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
  ('harvey_oak_entrance', 'Harvey Oak Supermart (Entrance)', 'Automatic doors and a welcome mat. Shopping baskets by the door.', 320, 480),
  ('harvey_oak_aisle', 'Harvey Oak Supermart (Aisle)', 'Shelves stocked with food, drinks, and everyday supplies. Prices are clearly marked.', 288, 448),
  ('harvey_oak_checkout', 'Harvey Oak Supermart (Checkout)', 'The checkout counter where Rosie rings up purchases. A register and a tip jar sit on the counter.', 352, 448),
  ('oak_classroom_a', 'Oak Hill College - Classroom A', 'A lecture hall where Klaus teaches. Rows of chairs face a chalkboard.', 896, 128),
  ('oak_staff_office', 'Oak Hill College - Staff Office', 'A cluttered office with stacks of papers and two desks.', 960, 128),
  ('smallville_library_reading_room', 'Smallville Library Reading Room', 'A quiet public reading room with long tables, bulletin flyers, and a window facing Ville Park.', 832, 288),
  ('smallville_library_archive', 'Smallville Library Archive', 'A compact archive room of town records, old newspapers, maps, and local-history boxes.', 896, 288),
  ('miller_community_garden', 'Miller Community Garden', 'A small shared garden with vegetable beds, flower rows, and a tool shed near the park path.', 736, 480),
  ('riverside_clinic_lobby', 'Riverside Clinic Lobby', 'A modest neighborhood clinic lobby with a check-in desk, worn chairs, and health pamphlets.', 576, 448),
  ('townhall_mayor_office', "Mayor's Office", 'The mayor works here. A large desk, town records, and a gavel.', 640, 320),
  ('townhall_assembly', 'Assembly Hall', 'A public meeting space with rows of chairs and a podium for debates.', 640, 288),
  ('townhall_civic_board', 'Civic Board Room', 'Complaints, hall of fame, ordinances, and official announcements are posted here.', 672, 304),
  ('townhall_voting_booth', 'Voting Booth', 'A small booth where citizens cast their ballots for mayor.', 608, 304)
ON CONFLICT (id) DO UPDATE
SET name = EXCLUDED.name,
    description = EXCLUDED.description,
    map_x = EXCLUDED.map_x,
    map_y = EXCLUDED.map_y;
