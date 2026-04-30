# Testing Guide (letta-city-sim)

Canonical project-level testing guide for **World API**, **lcity CLI**, and simulation behavior.

> LettaBot-specific testing still lives in `lettabot/TESTING.md`.

## 0) Prerequisites

- Docker Desktop running
- Postgres container up
- Seed data loaded
- `SIM_API_KEY` available for authenticated routes

```powershell
docker compose up db -d
powershell -ExecutionPolicy Bypass -File .\scripts\seed.ps1

cd world-api
$env:DATABASE_URL="postgres://sim:sim_dev_password@localhost:5432/letta_city_sim"
cargo run
```

Or from repo root:

```powershell
docker compose up world-api frontend -d
```

---

## 1) Health + world time

```powershell
curl.exe http://localhost:3001/health
curl.exe http://localhost:3001/world/time
```

Expect:
- `/health` => `ok`
- `/world/time` => includes simulation time fields (`timestamp`, `time_of_day`, `simulation_paused`)

---

## 2) Locations + pathfinding

```powershell
curl.exe http://localhost:3001/locations
curl.exe http://localhost:3001/locations/lin_kitchen
curl.exe http://localhost:3001/locations/lin_kitchen/nearby
curl.exe "http://localhost:3001/pathfind?from=lin_bedroom&to=hobbs_cafe_seating"
```

Expect:
- Non-empty locations list
- Location detail includes nearby data
- Pathfind returns `path` + `travel_time_seconds`

---

## 3) Agents read + move + activity

```powershell
$env:SIM_API_KEY="devkey"

curl.exe http://localhost:3001/agents
curl.exe http://localhost:3001/agents/eddy_lin

curl.exe -X PATCH http://localhost:3001/agents/move ^
  -H "Content-Type: application/json" ^
  -H "x-agent-id: eddy_lin" ^
  -H "x-sim-key: $env:SIM_API_KEY" ^
  -d "{\"location_id\":\"lin_kitchen\"}"

curl.exe -X PATCH http://localhost:3001/agents/eddy_lin/activity ^
  -H "Content-Type: application/json" ^
  -H "x-sim-key: $env:SIM_API_KEY" ^
  -d "{\"activity\":\"Cooking lunch\"}"

curl.exe -X DELETE http://localhost:3001/agents/eddy_lin/activity ^
  -H "x-sim-key: $env:SIM_API_KEY"
```

Expect:
- Agent state transitions (`walking`, `working`, `idle`)
- 400 for missing/invalid `x-agent-id` on `/agents/move`

---

## 3.5) Jobs catalog + assignment

Validate the seeded town/meta jobs and assignment endpoints.

```powershell
curl.exe http://localhost:3001/jobs
curl.exe http://localhost:3001/jobs/dispatcher
curl.exe http://localhost:3001/jobs/music_student/agents
curl.exe http://localhost:3001/agents/eddy_lin/jobs

curl.exe -X PATCH http://localhost:3001/agents/eddy_lin/jobs/writer ^
  -H "Content-Type: application/json" ^
  -H "x-sim-key: $env:SIM_API_KEY" ^
  -d "{\"is_primary\":false,\"notes\":\"Docs support\"}"

curl.exe -X DELETE http://localhost:3001/agents/eddy_lin/jobs/writer ^
  -H "x-sim-key: $env:SIM_API_KEY"
```

Expect:
- `/jobs` includes both `town` and `meta` roles
- starter agents have seeded primary town jobs (`music_student`, `cafe_owner`, etc.)
- assignment and removal append events without mutating prior event history
- assigning `--primary` through the API leaves at most one primary job for an agent

CLI smoke tests:

```powershell
node .\lcity\bin\lcity.mjs list_jobs
node .\lcity\bin\lcity.mjs get_job --id dispatcher
node .\lcity\bin\lcity.mjs list_agent_jobs --agent-id eddy_lin
node .\lcity\bin\lcity.mjs assign_job --agent-id eddy_lin --job-id writer --notes "Docs support"
node .\lcity\bin\lcity.mjs remove_job --agent-id eddy_lin --job-id writer
```

---

## 4) Inventory core + transfer adjacency

```powershell
curl.exe http://localhost:3001/inventory/eddy_lin

curl.exe -X PATCH http://localhost:3001/inventory/eddy_lin/add ^
  -H "Content-Type: application/json" ^
  -H "x-sim-key: $env:SIM_API_KEY" ^
  -d "{\"item_id\":\"sheet_music_001\"}"

curl.exe -X PATCH http://localhost:3001/inventory/eddy_lin/remove ^
  -H "Content-Type: application/json" ^
  -H "x-sim-key: $env:SIM_API_KEY" ^
  -d "{\"item_id\":\"sheet_music_001\"}"

curl.exe -X PATCH http://localhost:3001/agents/eddy_lin/inventory/transfer ^
  -H "Content-Type: application/json" ^
  -H "x-sim-key: $env:SIM_API_KEY" ^
  -d "{\"to_agent_id\":\"abigail_chen\",\"item_id\":\"sheet_music_001\"}"
```

Expect:
- Add/remove respects ownership/location rules
- Transfer works only for directly adjacent agents

---

## 5) Stackable consumables (`use-item`)

Validate stack decrement + vitals behavior.

### 5.1 Prepare item metadata

Ensure an inventory row has:
- `quantity = 3`
- `consumable_type = 'food'` (or `water`, `stamina`, `sleep`)
- `vital_value = 10`

### 5.2 Consume item

```powershell
curl.exe -X POST http://localhost:3001/agents/use-item ^
  -H "Content-Type: application/json" ^
  -H "x-agent-id: eddy_lin" ^
  -H "x-sim-key: $env:SIM_API_KEY" ^
  -d "{\"item_id\":\"apple_001\",\"quantity\":1}"
```

Expect:
- 200 response
- Quantity decremented
- Row deleted when quantity reaches 0
- Matching vital increases and is clamped to 100
- Event logged as `item.used`

### 5.3 Error checks

- quantity <= 0 => 400
- quantity requested > quantity owned => 400
- item not owned by agent => 400

---

## 6) Economy endpoint

```powershell
curl.exe -X PATCH http://localhost:3001/agents/eddy_lin/economy ^
  -H "Content-Type: application/json" ^
  -H "x-agent-id: eddy_lin" ^
  -H "x-sim-key: $env:SIM_API_KEY" ^
  -d "{\"amount_cents\":250,\"reason\":\"Quest reward\"}"
```

Expect:
- Positive amount updates income metadata
- Negative amount updates expense metadata
- Event type `economy.credit` or `economy.debit`

---

## 7) Board, objects, and events

```powershell
curl.exe http://localhost:3001/board
curl.exe http://localhost:3001/board/posts

curl.exe -X PATCH http://localhost:3001/board/posts ^
  -H "Content-Type: application/json" ^
  -H "x-agent-id: maria_lopez" ^
  -H "x-sim-key: $env:SIM_API_KEY" ^
  -d "{\"text\":\"Town hall at 6 PM\"}"

curl.exe http://localhost:3001/locations/lin_kitchen/objects

curl.exe -X PATCH http://localhost:3001/objects/stove_lin_kitchen ^
  -H "Content-Type: application/json" ^
  -H "x-agent-id: eddy_lin" ^
  -H "x-sim-key: $env:SIM_API_KEY" ^
  -d "{\"state\":{\"on\":true}}"

curl.exe "http://localhost:3001/events?limit=20"
```

Expect:
- Board privacy behavior unchanged (`/board` omits actor identity)
- Object state updates persist and emit events
- Event filters work (`since/location_id/actor_id/type/limit`)

---

## 8) lcity CLI tests

```powershell
$env:SIM_API_KEY="devkey"
New-Item -ItemType Directory -Force .lcity | Out-Null
Set-Content .lcity\agent_id "eddy_lin"

node .\lcity\bin\lcity.mjs health_check
node .\lcity\bin\lcity.mjs list_inventory
node .\lcity\bin\lcity.mjs use_item --item-id apple_001 --quantity 1
node .\lcity\bin\lcity.mjs economy_update --amount-cents 100 --reason "bonus"
```

Expect:
- JSON output for all commands
- `use_item` and `economy_update` reflect backend state changes

---

## 9) Sleep interaction

Validate room-level sleep behavior using a seeded bed object.

### 9.1 Ensure seeded bed exists

After seeding, confirm the bedroom has a sleep-capable bed object:

```powershell
curl.exe http://localhost:3001/locations/lin_bedroom/objects
```

Expect a bed object such as `bed_lin_bedroom` with `occupied_by: null` and `sleep` in `actions`.

### 9.2 Put the agent in the bedroom and start sleep

```powershell
$env:SIM_API_KEY="devkey"

curl.exe -X PATCH http://localhost:3001/agents/move ^
  -H "Content-Type: application/json" ^
  -H "x-agent-id: eddy_lin" ^
  -H "x-sim-key: $env:SIM_API_KEY" ^
  -d "{\"location_id\":\"lin_bedroom\"}"

curl.exe -X POST http://localhost:3001/agents/sleep ^
  -H "x-agent-id: eddy_lin" ^
  -H "x-sim-key: $env:SIM_API_KEY"
```

Expect:
- 200 response
- agent state becomes `sleeping`
- `current_activity` becomes `Sleeping`
- bed state updates to `occupied_by = eddy_lin`
- event logged as `agent.sleep.started`

### 9.3 Wake the agent

```powershell
curl.exe -X DELETE http://localhost:3001/agents/sleep ^
  -H "x-agent-id: eddy_lin" ^
  -H "x-sim-key: $env:SIM_API_KEY"
```

Expect:
- 200 response
- agent state becomes `idle`
- `current_activity` clears
- bed state returns to `occupied_by = null`
- event logged as `agent.sleep.ended`

### 9.4 CLI sleep/wake commands

```powershell
$env:SIM_API_KEY="devkey"
Set-Content .lcity\agent_id "eddy_lin"

node .\lcity\bin\lcity.mjs sleep
node .\lcity\bin\lcity.mjs wake_up
```

Expect:
- both commands return machine-readable JSON
- notifications describe sleep/wake transitions

### 9.5 Error checks

- trying to sleep outside a room with a usable bed => 400
- trying to sleep while already sleeping => 400
- trying to wake while not sleeping => 400

---

## 10) Interrupt / wake pipeline

Start the local daemon and verify both event-driven and manual interrupts pass through the same path.

```powershell
$env:SIM_API_KEY="devkey"
$env:LETTABOT_API_KEY="user-api-key"
lcity daemon --start

# manual interrupt
lcity lettabot_notify --message "Wake up and inspect the board"
```

Then inspect `.lcity/daemon.log`.

Expect:
- log lines use the unified `interrupt` wording
- manual notify logs include `cause=manual_message`
- websocket-driven wakes log `cause=<event_type>` and `transport=lettabot_completion`

---

## 11) Regression checklist

- Unknown IDs => 404 where expected
- Invalid payloads => 400 where expected
- Missing `x-agent-id` on required routes => 400
- Inventory add/remove/transfer still works
- No command regressions in `lcity`
