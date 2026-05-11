-- Time of day system

-- Shop hours (sim hours, 0-23)
ALTER TABLE shops ADD COLUMN opens_at SMALLINT NOT NULL DEFAULT 7;
ALTER TABLE shops ADD COLUMN closes_at SMALLINT NOT NULL DEFAULT 21;

-- Update Harvey Oak: 7am - 9pm
UPDATE shops SET opens_at = 7, closes_at = 21 WHERE id = 'harvey_oak';

-- Update Hobbs Cafe: 6am - 6pm
UPDATE shops SET opens_at = 6, closes_at = 18 WHERE id = 'hobbs_cafe';

-- Sim clock configuration
INSERT INTO simulation_state (id, key, value)
VALUES ('time_config', 'time', '{"time_scale": 12.0, "paused": false}')
ON CONFLICT (id) DO UPDATE SET value = EXCLUDED.value;
