INSERT INTO world_objects (id, name, location_id, state, actions)
VALUES
  ('bed_lin_bedroom', 'Eddy''s Bed', 'lin_bedroom', '{"occupied_by": null}', ARRAY['sleep']),
  ('stove_lin_kitchen', 'Lin Kitchen Stove', 'lin_kitchen', '{"on": false}', ARRAY['turn_on', 'turn_off', 'cook']),
  ('piano_lin_living', 'Living Room Piano', 'lin_living_room', '{"in_use": false}', ARRAY['play', 'practice']),
  ('coffee_machine_hobbs', 'Hobbs Coffee Machine', 'hobbs_cafe_counter', '{"on": true}', ARRAY['brew', 'clean']),
  ('notice_board_main', 'Notice Board', 'notice_board', '{"posts": []}', ARRAY['post', 'read']),
  ('bench_park_east', 'East Park Bench', 'ville_park_east', '{}', ARRAY['sit']),
  ('bench_park_west', 'West Park Bench', 'ville_park_west', '{}', ARRAY['sit']),
  ('shelf_food_harvey', 'Food Shelf', 'harvey_oak_aisle', '{"category": "food"}', ARRAY['browse', 'buy']),
  ('shelf_drinks_harvey', 'Drinks Shelf', 'harvey_oak_aisle', '{"category": "drinks"}', ARRAY['browse', 'buy']),
  ('shelf_supplies_harvey', 'Supplies Shelf', 'harvey_oak_aisle', '{"category": "supplies"}', ARRAY['browse', 'buy']),
  ('checkout_counter_harvey', 'Checkout Counter', 'harvey_oak_checkout', '{"register_open": true, "last_cleaned_at": null}', ARRAY['buy']),
  ('delivery_crate_harvey', 'Delivery Crate', 'harvey_oak_checkout', '{"delivery_pending": false}', ARRAY['receive']),
  ('delivery_crate_hobbs', 'Delivery Crate', 'hobbs_cafe_kitchen', '{"delivery_pending": false}', ARRAY['receive'])
ON CONFLICT (id) DO UPDATE
SET name = EXCLUDED.name,
    location_id = EXCLUDED.location_id,
    state = EXCLUDED.state,
    actions = EXCLUDED.actions;

INSERT INTO inventory_items (id, name, held_by, location_id, state, quantity, consumable_type, vital_value, price_cents)
VALUES
  ('coffee_beans_001', 'Coffee Beans', NULL, 'hobbs_cafe_kitchen', '{}', 1, NULL, NULL, NULL),
  ('sheet_music_001', 'Sheet Music', NULL, 'lin_bedroom', '{}', 1, NULL, NULL, NULL),
  ('paint_brush_001', 'Paint Brush', NULL, 'ville_park_west', '{}', 1, NULL, NULL, NULL),
  ('notebook_001', 'Lecture Notebook', NULL, 'oak_classroom_a', '{}', 1, NULL, NULL, NULL),
  ('bread_001', 'Bread Loaf', NULL, 'harvey_oak_aisle', '{}', 5, 'food', 25, 150),
  ('sandwich_001', 'Sandwich', NULL, 'harvey_oak_aisle', '{}', 3, 'food', 40, 350),
  ('apple_001', 'Apple', NULL, 'harvey_oak_aisle', '{}', 5, 'food', 15, 100),
  ('water_bottle_001', 'Water Bottle', NULL, 'harvey_oak_aisle', '{}', 5, 'water', 30, 150),
  ('coffee_can_001', 'Coffee Can', NULL, 'harvey_oak_aisle', '{}', 3, 'stamina', 20, 400),
  ('energy_bar_001', 'Energy Bar', NULL, 'harvey_oak_aisle', '{}', 4, 'stamina', 25, 250),
  ('soap_001', 'Soap Bar', NULL, 'harvey_oak_aisle', '{}', 5, 'hygiene', 30, 150),
  ('shampoo_001', 'Shampoo', NULL, 'harvey_oak_aisle', '{}', 4, 'hygiene', 20, 200),
  ('deodorant_001', 'Deodorant', NULL, 'harvey_oak_aisle', '{}', 4, 'hygiene', 15, 300),
  ('perfume_001', 'Perfume', NULL, 'harvey_oak_aisle', '{}', 3, 'appearance', 20, 800),
  ('cologne_001', 'Cologne', NULL, 'harvey_oak_aisle', '{}', 3, 'appearance', 20, 800),
  ('makeup_001', 'Makeup Kit', NULL, 'harvey_oak_aisle', '{}', 2, 'appearance', 25, 1200),
  ('backroom_bread_001', 'Bread Loaf', NULL, 'harvey_oak_checkout', '{"backroom": true, "restock_price": 150}', 5, 'food', 25, NULL),
  ('backroom_water_001', 'Water Bottle', NULL, 'harvey_oak_checkout', '{"backroom": true, "restock_price": 150}', 5, 'water', 30, NULL),
  ('backroom_apple_001', 'Apple', NULL, 'harvey_oak_checkout', '{"backroom": true, "restock_price": 100}', 5, 'food', 15, NULL),
  ('backroom_soap_001', 'Soap Bar', NULL, 'harvey_oak_checkout', '{"backroom": true, "restock_price": 150}', 5, 'hygiene', 30, NULL),
  ('backroom_shampoo_001', 'Shampoo', NULL, 'harvey_oak_checkout', '{"backroom": true, "restock_price": 200}', 4, 'hygiene', 20, NULL),
  ('backroom_deodorant_001', 'Deodorant', NULL, 'harvey_oak_checkout', '{"backroom": true, "restock_price": 300}', 4, 'hygiene', 15, NULL),
  ('backroom_perfume_001', 'Perfume', NULL, 'harvey_oak_checkout', '{"backroom": true, "restock_price": 800}', 3, 'appearance', 20, NULL),
  ('backroom_cologne_001', 'Cologne', NULL, 'harvey_oak_checkout', '{"backroom": true, "restock_price": 800}', 3, 'appearance', 20, NULL),
  ('backroom_makeup_001', 'Makeup Kit', NULL, 'harvey_oak_checkout', '{"backroom": true, "restock_price": 1200}', 2, 'appearance', 25, NULL),
  -- Hobbs Cafe shelf items
  ('hobbs_coffee_001', 'Fresh Coffee', NULL, 'hobbs_cafe_counter', '{}', 5, 'stamina', 25, 300),
  ('hobbs_pastry_001', 'Butter Croissant', NULL, 'hobbs_cafe_counter', '{}', 4, 'food', 20, 250),
  ('hobbs_tea_001', 'Herbal Tea', NULL, 'hobbs_cafe_counter', '{}', 3, 'water', 15, 200),
  -- Hobbs Cafe backroom items
  ('backroom_hobbs_coffee_001', 'Coffee Beans', NULL, 'hobbs_cafe_kitchen', '{"backroom": true, "restock_price": 300}', 5, 'stamina', 25, NULL),
  ('backroom_hobbs_pastry_001', 'Croissant Dough', NULL, 'hobbs_cafe_kitchen', '{"backroom": true, "restock_price": 250}', 4, 'food', 20, NULL),
  ('backroom_hobbs_tea_001', 'Tea Leaves', NULL, 'hobbs_cafe_kitchen', '{"backroom": true, "restock_price": 200}', 3, 'water', 15, NULL)
ON CONFLICT (id) DO UPDATE
SET name = EXCLUDED.name,
    held_by = EXCLUDED.held_by,
    location_id = EXCLUDED.location_id,
    state = EXCLUDED.state;
