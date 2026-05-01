# letta-city-sim

Autonomous city simulation where each NPC is a Letta agent acting on its own clock. Architecture mirrors the PRD in `docs/letta-city-sim-prd.md`:

- **world-api/** &mdash; Rust/Axum REST service exposing world state.
- **frontend/** &mdash; Next.js 15 + Phaser 3 visualization.
- **townhall/** &mdash; Next.js community contribution board powered by GitHub Issues.
- **seed/** &mdash; JSON (or scripts) that populate Smallville.
- **docs/** &mdash; Product brief, extensive TODO, and archived plans.
- **scripts/** &mdash; Tooling helpers (migrations, bootstrap, etc.).

## Status
Backend MVP foundation is now live.

Implemented so far:
- World API scaffold with Axum + sqlx + Postgres migration
- Seed data + idempotent seeding script (`scripts/seed.ps1`)
- Agents API (list/detail/move/activity)
- Jobs API + seeded town/meta role catalog (`jobs`, `agent_jobs`)
- Locations API (list/detail/nearby)
- Pathfinding API (`GET /pathfind` using Dijkstra)
- Inventory API (list/add/remove/adjacent-only transfer)
- Notice board API (public text-only + internal audit events)
- Objects API (`GET /locations/:location_id/objects`, `PATCH /objects/:id`)
- Events API (`GET /events`, `POST /events`)
- World time API (`GET /world/time`)
- Canonical QA checklist in `TESTING.md`
- Community contribution board scaffold in `townhall/`

Still pending: Letta SDK tool wiring, webhook bridge, conversations, websocket stream, and frontend map/UI.

## Local development
Dependencies:
- Docker / Docker Compose
- Rust toolchain (stable)
- Node.js 20 + pnpm (once frontend begins)

```powershell
# Start database
docker compose up db -d

# Seed world data
powershell -ExecutionPolicy Bypass -File .\scripts\seed.ps1

# Run API
cd world-api
$env:DATABASE_URL="postgres://sim:sim_dev_password@localhost:5432/letta_city_sim"
cargo run
```

> Keep `.env` synced with `.env.example`. Never commit real secrets.

## Bundled Docker hosting stack

There is now a bundled deployment path that packages:

- `world-api`
- `frontend`

into a single image with **one public frontend port**. The frontend proxies API and websocket traffic internally to the bundled world-api.

This is the easiest way to host the sim behind **one public port** while keeping Postgres internal to the Docker network.

### Build the bundled image

```powershell
docker build -f Dockerfile.bundle -t letta-city-sim-bundled .
```

### Host the full sim stack

The compose stack starts:

- `db` - Postgres with a named persistent volume
- `db-init` - a one-shot bootstrap step that applies migrations and idempotent seed SQL
- `app` - the bundled frontend + world-api image on one public port

```powershell
$env:SIM_API_KEY="change_me"
docker compose -f docker-compose.bundle.yml up --build -d
```

Then open:

- `http://localhost:3000` - bundled frontend
- `http://localhost:3000/api/health` - world-api health through the frontend proxy

Useful overrides:

```powershell
$env:SIM_API_KEY="change_me"
$env:POSTGRES_PASSWORD="change_me_too"
$env:PUBLIC_PORT="3000"
docker compose -f docker-compose.bundle.yml up --build -d
```

Notes:

- the Postgres container is **not** published publicly by default
- migrations are tracked in `schema_migrations`
- seed files are re-applied on bootstrap but are written idempotently with `ON CONFLICT`
- the database data lives in the named Docker volume `pgdata_bundle`

### Run the bundled app against an existing Postgres database

If you already have Postgres elsewhere, run the bootstrap step once and then start the bundled app:

```powershell
docker run --rm ^
  -e DATABASE_URL="postgres://sim:sim_dev_password@host.docker.internal:5432/letta_city_sim" ^
  letta-city-sim-bundled /app/scripts/db-bootstrap.sh
```

Then start the app:

```powershell
docker run --rm -p 3000:3000 ^
  -e DATABASE_URL="postgres://sim:sim_dev_password@host.docker.internal:5432/letta_city_sim" ^
  -e SIM_API_KEY="dev_key_change_me" ^
  letta-city-sim-bundled
```

## Quick endpoint smoke tests

```powershell
curl.exe http://localhost:3001/health
curl.exe http://localhost:3001/agents
curl.exe http://localhost:3001/jobs
curl.exe "http://localhost:3001/pathfind?from=lin_bedroom&to=hobbs_cafe_seating"
curl.exe http://localhost:3001/board
curl.exe http://localhost:3001/world/time
```

For full manual validation, use `TESTING.md`.

## Authentication & CLI helper

All mutating REST routes on `world-api` now require:

- `x-sim-key` header — matches `SIM_API_KEY` configured on the server.
- `x-agent-id` header — ID (or `letta_agent_id`) of the acting NPC.

Set the SIM key in your shell before running CLI commands:

```powershell
$env:SIM_API_KEY="dev_key_change_me"
node .\lcity\bin\lcity.mjs health_check
```

The CLI reads the agent ID from `.lcity/agent_id` (see `lcity/README.md`) and automatically attaches both headers for every request. Use `--sim-key <value>` per command if you prefer not to export the env var.

Sample write request (curl):

```powershell
curl.exe -X PATCH http://localhost:3001/board/posts ^
  -H "Content-Type: application/json" ^
  -H "x-sim-key: dev_key_change_me" ^
  -H "x-agent-id: eddy_lin" ^
  -d '{"text":"Town hall at 6 PM"}'
```

Read-only endpoints continue to work without headers.

## Letta Code skill

This repo includes a Letta Code skill at `skills/living-in-letta-city/` for controlling NPCs through the existing `lcity` CLI. Use it when an agent should act inside the city instead of calling the World API by hand.

Example:

```powershell
$env:SIM_API_KEY="dev_key_change_me"
$env:LCITY_API_BASE="http://localhost:3001"
node .\skills\living-in-letta-city\scripts\lcity-agent.mjs --agent-id eddy_lin health_check
node .\skills\living-in-letta-city\scripts\lcity-agent.mjs --agent-id eddy_lin move_to --location-id hobbs_cafe_seating
```

When running inside Letta Code, set `LCITY_AGENT_ID` explicitly for the city-sim NPC identity. Do not rely on the runtime `AGENT_ID`, because Letta agent ids and city-sim NPC ids are usually different.

## Documentation
- Contribution workflow: `CONTRIBUTING.md`
- Canonical product brief: `docs/letta-city-sim-prd.md`
- Full execution checklist: `docs/letta-city-sim-extensive-todo.md`
- Community contribution breakdown: `docs/community-contributions.md`
- Contributor guides index: `docs/guides/README.md`
- Jobs guide: `docs/guides/adding-jobs.md`
- Location guide: `docs/guides/adding-locations.md`
- Items/consumables guide: `docs/guides/adding-items-and-consumables.md`
- Playtesting guide: `docs/guides/playtesting.md`
- Historical docs live under `docs/archive/`
