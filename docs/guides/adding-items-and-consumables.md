# Adding items and consumables

This guide is for contributors who want to add **new item content** without changing the underlying architecture.

Right now, the safest community contribution path is:

- propose items and consumables
- add new seed-data content packs
- keep changes compatible with the current schema and placeholder vitals model

## What the current system supports

The current world supports consumable-like inventory items with fields such as:

- `quantity`
- `consumable_type`
- `vital_value`

Supported consumable categories today are conceptually:

- `food`
- `water`
- `stamina`
- `sleep`

The current vitals model is intentionally **placeholder/simple**.

That means contributors should focus on:

- sensible item names
- useful categories
- practical values
- coherent content packs

and **not** on inventing new physiology/economy formulas.

## Good contribution types

Examples of good community-safe work:

- add a set of cafe drinks
- add grocery pantry items
- add sleep-related consumables if they fit the current model
- add college/snack items
- turn approved item concepts into seed-data packs

## What not to change

Please do not redesign in a community PR:

- the item schema
- the vitals architecture
- wake/sleep semantics
- economy architecture

Those are maintainer-owned.

## Naming guidance

Item contributions are easier to review if they are:

- clear
- grounded in Smallville
- grouped by venue or category

Examples:

- `coffee_small`
- `ham_sandwich`
- `bottled_water`
- `campus_energy_bar`

Use readable, stable naming rather than joke/internal-only identifiers.

## Choosing values

Because the current vitals logic is placeholder-level:

- keep values conservative
- keep them easy to reason about
- do not try to perfectly balance the economy yet

Good rule of thumb:

- small consumables -> small boosts
- basic meals/drinks -> moderate boosts
- avoid extreme values unless there is a very obvious reason

## Suggested workflow

1. Start from a concept issue if one exists.
2. Group additions into a coherent pack.
3. Keep the PR focused.
4. Validate with the existing `use_item` flow.

## Example contribution packs

Good pack ideas:

- **Cafe pack:** coffee, tea, sandwich, pastry, juice
- **Grocery pack:** bread, apples, bottled water, noodles, soup
- **College snack pack:** chips, soda, granola bar, instant coffee

## Validation steps

After adding consumable data, validate locally using the current API/CLI flow.

### API-level use-item test

```powershell
$env:SIM_API_KEY="devkey"

curl.exe -X POST http://localhost:3001/agents/use-item ^
  -H "Content-Type: application/json" ^
  -H "x-agent-id: eddy_lin" ^
  -H "x-sim-key: $env:SIM_API_KEY" ^
  -d "{\"item_id\":\"apple_001\",\"quantity\":1}"
```

### CLI-level test

```powershell
$env:SIM_API_KEY="devkey"
Set-Content .lcity\agent_id "eddy_lin"
node .\lcity\bin\lcity.mjs use_item --item-id apple_001 --quantity 1
```

See `TESTING.md` for the broader validation checklist.

## Job proposals vs job implementation

Community contributors are welcome to:

- propose job catalogs
- define venue roles
- describe likely interactions/jobs unlocked by locations

But please treat full job-system architecture as maintainer-owned unless explicitly opened up.

So it is good to contribute:

- "bakery clerk"
- "college librarian"
- "park groundskeeper"

It is **not** the place to redesign:

- scheduling architecture
- wages/economy internals
- simulation lifecycle rules

## When to stop and ask a maintainer

Stop and ask before proceeding if your contribution needs:

- schema changes
- new consumable categories unsupported by the current system
- complex economy logic
- major vitals model changes

Those are maintainer-owned decisions.
