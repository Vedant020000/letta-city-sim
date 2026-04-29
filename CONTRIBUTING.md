# Contributing to letta-city-sim

Thanks for helping build `letta-city-sim`.

This project has two layers of contribution:

- **maintainer-owned work** — architecture, lifecycle semantics, wake/interrupt internals, auth/security, schema direction
- **community-open work** — content packs, locations, consumables, props, art, playtesting, docs, frontend polish, and bounded implementation tasks

If you are not sure whether a task is safe for community work, check `docs/community-contributions.md` first.

## Start here

1. Open the project board in `townhall/` or browse GitHub issues directly.
2. Pick an issue labeled `community`.
3. Claim it by commenting:

```text
/claim
```

4. If you stop working on it, release it with:

```text
/release
```

5. Open a PR when your work is ready.

## Good contribution lanes

Community contributors are especially welcome in these areas:

- **Docs:** setup guides, contributor guides, walkthroughs, examples
- **Content:** locations, jobs, consumables, notice board posts, venue ideas
- **Backend seed data:** additive locations, adjacency, objects, consumables
- **Frontend polish:** inspector panels, event feed improvements, map polish, controls, interaction affordances
- **Art/assets:** sprites, props, tiles, icons, visual direction
- **Playtesting:** long-running sessions, model comparisons, bug reports, logs

## Maintainer-owned areas

Please do **not** open PRs that redesign these without explicit maintainer direction:

- wake / interrupt architecture
- auth / security model
- schema direction / large migrations
- sleep / lifecycle semantics
- major frontend architecture changes
- other issues labeled `architecture-sensitive` or `maintainer-only`

## Local setup

### Minimum stack

- **Windows PowerShell** friendly workflow
- Docker Desktop (used mainly for Postgres)
- Rust toolchain
- Node.js 20+

### Backend / database

```powershell
docker compose up db -d
powershell -ExecutionPolicy Bypass -File .\scripts\seed.ps1

cd world-api
$env:DATABASE_URL="postgres://sim:sim_dev_password@localhost:5432/letta_city_sim"
$env:SIM_API_KEY="dev_key_change_me"
cargo run
```

### Frontend

```powershell
cd frontend
npm install
$env:NEXT_PUBLIC_API_URL="http://localhost:3001"
$env:NEXT_PUBLIC_WS_URL="ws://localhost:3001/ws/events"
npm run dev
```

### Townhall board app

```powershell
cd townhall
npm install
npm run dev
```

## Before you open a PR

At minimum, make sure your change is scoped and reviewable.

Helpful checks:

- docs-only change: proofread links and commands
- frontend change: run `npm run build` in the touched frontend app if practical
- Rust/backend change: run `cargo check` in `world-api`
- seed-data change: validate with `TESTING.md` and the relevant guide in `docs/guides/`

## PR expectations

Please keep PRs:

- **small and bounded**
- tied to a specific issue
- explicit about what changed
- explicit about how you tested it

Good PR descriptions include:

- issue number
- short summary
- screenshots if UI/art changed
- testing notes
- any follow-up work still needed

## Guides

Use these before starting a contribution:

- `docs/community-contributions.md`
- `docs/guides/README.md`
- `docs/guides/adding-locations.md`
- `docs/guides/adding-items-and-consumables.md`
- `docs/guides/playtesting.md`

## Important constraints

- The **PRD** in `docs/letta-city-sim-prd.md` is the canonical product direction.
- The **World API** is the source of truth for physical state.
- The browser frontend should consume **World API REST + `/ws/events`**, not Letta wake/daemon internals.
- Vitals logic is still intentionally placeholder-level.
- Avoid broad refactors unless a maintainer explicitly asks for them.

## Need ideas?

Look for issues labeled:

- `good first issue`
- `community`
- `help wanted`
- `docs`
- `content`
- `art`
- `playtest`
- `frontend`
- `backend`

Thanks for helping make Smallville stranger and better.
