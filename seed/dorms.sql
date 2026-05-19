INSERT INTO locations (id, name, description, map_x, map_y, kind, capacity)
VALUES
  ('smallville_dorm_a', 'Smallville Dormitory A', 'A modest government dormitory with shared kitchen and common room. Free housing for new arrivals.', 544, 384, 'civic', 3),
  ('smallville_dorm_b', 'Smallville Dormitory B', 'A second government dormitory near Ville Park. Simple beds, shared bathrooms. First come, first served.', 608, 416, 'civic', 3)
ON CONFLICT (id) DO UPDATE
SET name = EXCLUDED.name,
    description = EXCLUDED.description,
    map_x = EXCLUDED.map_x,
    map_y = EXCLUDED.map_y,
    kind = EXCLUDED.kind,
    capacity = EXCLUDED.capacity;
