# Manual API Test Checklist

Use this checklist after major backend changes.

## 0) Prerequisites

- Docker Desktop running
- Postgres container up
- Seed data loaded
- World API running (`cargo run` in `world-api/`)

Commands:

```powershell
docker compose up db -d
powershell -ExecutionPolicy Bypass -File .\scripts\seed.ps1

cd world-api
$env:DATABASE_URL="postgres://sim:sim_dev_password@localhost:5432/letta_city_sim"
cargo run
```

---

## 1) Health + time

```powershell
curl.exe http://localhost:3001/health
curl.exe http://localhost:3001/world/time
```

Expect:
- `/health` => `ok`
- `/world/time` => `timestamp`, `time_of_day`, `simulation_paused`

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
- Location detail includes `nearby`
- Pathfind returns `path` + `travel_time_seconds`

---

## 3) Agents read + move + activity

```powershell
curl.exe http://localhost:3001/agents
curl.exe http://localhost:3001/agents/eddy_lin

curl.exe -X PATCH http://localhost:3001/agents/move ^
  -H "Content-Type: application/json" ^
  -H "x-agent-id: eddy_lin" ^
  -d "{\"location_id\":\"lin_kitchen\"}"

curl.exe -X PATCH http://localhost:3001/agents/eddy_lin/activity ^
  -H "Content-Type: application/json" ^
  -d "{\"activity\":\"Cooking lunch\"}"

curl.exe -X DELETE http://localhost:3001/agents/eddy_lin/activity
```

Expect:
- Agent state updates correctly (`walking`, `working`, `idle`)
- 400 on missing/invalid `x-agent-id` for `/agents/move`

---

## 4) Inventory core + transfer adjacency rule

```powershell
curl.exe http://localhost:3001/inventory/eddy_lin

curl.exe -X PATCH http://localhost:3001/inventory/eddy_lin/add ^
  -H "Content-Type: application/json" ^
  -d "{\"item_id\":\"sheet_music_001\"}"

curl.exe -X PATCH http://localhost:3001/inventory/eddy_lin/remove ^
  -H "Content-Type: application/json" ^
  -d "{\"item_id\":\"sheet_music_001\"}"

curl.exe -X PATCH http://localhost:3001/agents/eddy_lin/inventory/transfer ^
  -H "Content-Type: application/json" ^
  -d "{\"to_agent_id\":\"abigail_chen\",\"item_id\":\"sheet_music_001\"}"
```

Expect:
- Add/remove works only with valid ownership/location
- Transfer only works when agents are directly adjacent (not same-place fallback)

---

## 5) Notice board privacy behavior

```powershell
curl.exe http://localhost:3001/board
curl.exe http://localhost:3001/board/posts

curl.exe -X PATCH http://localhost:3001/board/posts ^
  -H "Content-Type: application/json" ^
  -H "x-agent-id: maria_lopez" ^
  -d "{\"text\":\"Town hall at 6 PM\"}"

curl.exe http://localhost:3001/board
curl.exe http://localhost:3001/board/posts

:: Use post_id from /board/posts
curl.exe -X DELETE http://localhost:3001/board/posts/<post_id> ^
  -H "x-agent-id: sam_moore"

curl.exe -X DELETE http://localhost:3001/board/clear ^
  -H "x-agent-id: sam_moore"
```

Expect:
- `/board` returns text only (no actor identity)
- `/board/posts` includes IDs for safe deletion
- create/delete/clear require `x-agent-id`

---

## 6) Objects + events

```powershell
curl.exe http://localhost:3001/objects/lin_kitchen

curl.exe -X PATCH http://localhost:3001/objects/stove_lin_kitchen ^
  -H "Content-Type: application/json" ^
  -H "x-agent-id: eddy_lin" ^
  -d "{\"state\":{\"on\":true}}"

curl.exe "http://localhost:3001/events?limit=20"

curl.exe -X POST http://localhost:3001/events ^
  -H "Content-Type: application/json" ^
  -d "{\"type\":\"weather.changed\",\"description\":\"Light rain started\",\"location_id\":\"ville_park_east\",\"metadata\":{\"intensity\":\"light\"}}"
```

Expect:
- Object update persists and logs event
- Event filters work by `since/location_id/actor_id/type/limit`

---

## 7) Error behavior spot checks

- Unknown IDs => 404 where expected
- Bad request payloads => 400 where expected
- Missing `x-agent-id` on required routes => 400

---

## 8) Shared lcity CLI health check

```powershell
New-Item -ItemType Directory -Force .lcity | Out-Null
Set-Content .lcity\agent_id "eddy_lin"
node .\lcity\bin\lcity.mjs health_check
```

Expect:
- JSON output with `ok: true` and `status_code: 200`
- `data.agent_id` matches input
- non-zero exit code + `ok: false` for invalid agent IDs
