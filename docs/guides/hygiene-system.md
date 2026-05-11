# Hygiene & Appearance System

Agents have two additional vitals that track personal cleanliness and grooming. These decay over time and affect how other agents perceive them — and how the world reacts to them.

---

## New Vitals

| Vital | Range | Decay Rate | Notes |
|-------|-------|-----------|-------|
| `hygiene_level` | 0–100 | 0.2/min (0.1/min while sleeping) | How clean the agent is |
| `appearance_level` | 0–100 | 0.15/min (0.075/min while sleeping) | How put-together they look |

### Hygiene/Appearance interaction

Appearance decay is modified by hygiene level:

| Hygiene | Appearance Decay Modifier |
|---------|--------------------------|
| < 20 | 2x faster (you can't look good if you smell) |
| 20–80 | Normal rate |
| > 80 | 0.5x slower (cleanliness helps) |

---

## Morning Routine Actions

These are the core hygiene actions agents use to start their day or freshen up:

| Tool | Where | Effect | Stamina Cost |
|------|-------|--------|-------------|
| `wash_up` | Water-access locations | +15 hygiene, +5 appearance | 5 |
| `shower` | Home only | +50 hygiene, +15 appearance | 15 |
| `brush_teeth` | Home only | +10 hygiene | 2 |
| `get_ready` | Home only | +60 hygiene, +40 appearance | 25 |
| `bathe` | Park/garden | +40 hygiene, +5 appearance | 10 |
| `swim` | Park/garden | +30 hygiene, -5 appearance, -20 stamina | 20 |
| `groom` | Anywhere | +15 appearance | 5 |

### Location access

| Location type | Water access | Home | Bathing |
|--------------|-------------|------|---------|
| `lin_*` (Lin house) | Yes | Yes | No |
| `hobbs_cafe_*` | Yes | No | No |
| `riverside_clinic_*` | Yes | No | No |
| `ville_park_*` | Yes | No | Yes |
| `miller_community_garden` | Yes | No | Yes |
| All others | No | No | No |

---

## Hygiene & Appearance Consumables

All consumables are used through the `use_item` tool — there are no special-purpose apply tools. The consumable type determines which vital gets boosted.

| Item | Consumable Type | Vital Boost | Shelf Price |
|------|----------------|-------------|-------------|
| Soap Bar | `hygiene` | +30 | $1.50 |
| Shampoo | `hygiene` | +20 | $2.00 |
| Deodorant | `hygiene` | +15 | $3.00 |
| Perfume | `appearance` | +20 | $8.00 |
| Cologne | `appearance` | +20 | $8.00 |
| Makeup Kit | `appearance` | +25 | $12.00 |

All items are available at Harvey Oak Supermarket. Rosie can restock them from the backroom like any other item.

### How consumable types map to vitals

| `consumable_type` | Vital restored |
|-------------------|---------------|
| `food` | `food_level` |
| `water` | `water_level` |
| `stamina` | `stamina_level` |
| `sleep` | `sleep_level` |
| `hygiene` | `hygiene_level` |
| `appearance` | `appearance_level` |

---

## Social Perception

### look_around

When an agent looks around, other agents' `hygiene_level` and `appearance_level` are included in the response. This lets agents notice who looks messy or well-groomed.

### speak_to

When speaking to another agent, the response includes an `appearance_context` field describing the target's visible state:

| Hygiene | Description |
|---------|------------|
| < 20 | "looks unkempt and could really use a wash" |
| 20–50 | "looks a bit disheveled" |
| > 80 | "looks fresh and clean" |

| Appearance | Description |
|-----------|------------|
| < 20 | "looks very messy" |
| 20–50 | "looks a bit untidy" |
| > 80 | "looks sharp and well-groomed" |

Multiple observations are combined, e.g. "looks fresh and clean; looks sharp and well-groomed".

---

## Database Schema

### Migration 0014: Hygiene System

```sql
ALTER TABLE agents ADD COLUMN hygiene_level SMALLINT NOT NULL DEFAULT 100;
ALTER TABLE agents ADD COLUMN appearance_level SMALLINT NOT NULL DEFAULT 100;
```

---

## Manifest Conditionals

| Tool | Condition |
|------|-----------|
| `groom` | Always available |
| `wash_up` | At water-access location |
| `shower` | At home location |
| `brush_teeth` | At home location |
| `get_ready` | At home location |
| `bathe` | At park/garden |
| `swim` | At park/garden |

No special manifest entries for consumables — `use_item` is always available, and agents will naturally use hygiene/appearance items when they have them.
