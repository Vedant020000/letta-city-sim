# Adding locations and adjacency edges

This guide is for contributors who want to expand Smallville with **new locations** and **matching adjacency/travel-time data**.

This is community-safe work as long as you are **adding to the existing model**, not redesigning it.

## What you are editing

The current seed files live in `seed/`:

- `seed/locations.sql`
- `seed/adjacency.sql`
- `seed/objects.sql`
- `seed/agents.sql`

For location expansion work, the primary files are:

- `seed/locations.sql`
- `seed/adjacency.sql`

## Current model

Locations currently have fields like:

- `id`
- `name`
- `description`
- `map_x`
- `map_y`

Adjacency data defines directed travel relationships with a travel time in seconds.

That means when you add a location, you usually also need to add one or more adjacency edges so:

- pathfinding works
- nearby lookups work
- the frontend can place the location meaningfully

## Rules of thumb

### 1. Keep additions coherent
Prefer adding:

- one small location cluster
- one venue plus nearby walkable connections
- one neighborhood-style expansion

Avoid adding scattered random locations with no clear relationship to the current town.

### 2. Use stable ids
Location IDs should be:

- lowercase
- underscore-separated
- descriptive

Examples:

- `oak_hill_library`
- `ville_bakery`
- `park_gazebo`

### 3. Keep travel times believable
Use travel times that make sense relative to nearby existing edges.

Do not introduce huge inconsistencies like:

- a very distant place with a tiny travel time
- a clearly adjacent place with a huge travel time

### 4. Add `map_x` / `map_y`
The frontend MVP already uses location coordinates for the placeholder town map.

So every new location should include sensible `map_x` / `map_y` values that fit the current layout.

## Suggested workflow

1. Pick an approved concept from a content issue or propose one in GitHub first.
2. Add the location row(s) in `seed/locations.sql`.
3. Add matching adjacency edges in `seed/adjacency.sql`.
4. Re-seed locally.
5. Validate with nearby lookup + pathfinding.

## Example contribution shape

Good example:

- add `ville_bakery`
- connect it to `hobbs_cafe_seating` and `ville_park`
- add realistic travel times
- later add matching props/objects in a separate or follow-up change

## Re-seeding locally

```powershell
docker compose up db -d
powershell -ExecutionPolicy Bypass -File .\scripts\seed.ps1
```

## Validation steps

After editing seed files, validate:

### 1. Locations list includes your new location

```powershell
curl.exe http://localhost:3001/locations
```

### 2. Location detail works

```powershell
curl.exe http://localhost:3001/locations/<your_location_id>
```

### 3. Nearby list makes sense

```powershell
curl.exe http://localhost:3001/locations/<your_location_id>/nearby
```

### 4. Pathfinding still works

```powershell
curl.exe "http://localhost:3001/pathfind?from=lin_bedroom&to=<your_location_id>"
```

Also test at least one path between your new location and another sensible nearby location.

## Common mistakes to avoid

- adding a location with no adjacency edges
- adding edges but forgetting the location row itself
- using coordinates that overlap existing areas badly
- using inconsistent ids/naming
- changing existing architecture instead of adding data

## When to stop and ask a maintainer

Stop and ask before proceeding if your change requires:

- new location schema fields
- backend pathfinding redesign
- coordinate-system redesign
- map rendering architecture changes

Those are maintainer-owned decisions.
