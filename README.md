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
- Locations API (list/detail/nearby)
- Pathfinding API (`GET /pathfind` using Dijkstra)
- Inventory API (list/add/remove/adjacent-only transfer)
- Notice board API (public text-only + internal audit events)
- Objects API (`GET /objects/:location_id`, `PATCH /objects/:id`)
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

## Optional bundled Docker image

There is now an optional bundled deployment/demo path that packages:

- `world-api`
- `frontend`

into a single image with **one public frontend port**. The frontend proxies API and websocket traffic internally to the bundled world-api.

This is meant for demos/deployment convenience, not as the preferred local workflow on Vedant's machine.

### Build the bundled image

```powershell
docker build -f Dockerfile.bundle -t letta-city-sim-bundled .
```

### Run against an existing Postgres database

```powershell
docker run --rm -p 3000:3000 ^
  -e DATABASE_URL="postgres://sim:sim_dev_password@host.docker.internal:5432/letta_city_sim" ^
  -e SIM_API_KEY="dev_key_change_me" ^
  letta-city-sim-bundled
```

This expects the target database to already have the current migrations and seed data applied.

### Demo compose path

For a one-command demo stack with a separate Postgres service, use:

```powershell
docker compose -f docker-compose.bundle.yml up --build
```

That compose file initializes Postgres with the current migrations + seed SQL and serves the bundled app on `http://localhost:3000`.

## Quick endpoint smoke tests

```powershell
curl.exe http://localhost:3001/health
curl.exe http://localhost:3001/agents
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

When running inside Letta Code, the wrapper can use the runtime `AGENT_ID` env var automatically. Set `LCITY_AGENT_ID` to override the city identity when the sim id differs from the Letta agent id.

## Documentation
- Contribution workflow: `CONTRIBUTING.md`
- Canonical product brief: `docs/letta-city-sim-prd.md`
- Full execution checklist: `docs/letta-city-sim-extensive-todo.md`
- Community contribution breakdown: `docs/community-contributions.md`
- Contributor guides index: `docs/guides/README.md`
- Location guide: `docs/guides/adding-locations.md`
- Items/consumables guide: `docs/guides/adding-items-and-consumables.md`
- Playtesting guide: `docs/guides/playtesting.md`
- Historical docs live under `docs/archive/`
