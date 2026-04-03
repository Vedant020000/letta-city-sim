# lcity CLI (Node.js)

Shared command-line tool layer for all Letta agents in `letta-city-sim`.

This package is structured to be published independently.

- entrypoint: `bin/lcity.mjs`
- command implementation: `src/cli.mjs`
- package metadata: `package.json`

## Run directly

```powershell
node .\lcity\bin\lcity.mjs health_check
```

## Supported commands (current)

- `health_check`
- `move_to --location-id`
- `move_to_agent --target-agent-id`
- `list_locations`, `get_location --id`, `nearby_locations --id`
- `pathfind --from --to`
- `world_time`
- `list_inventory`
- `board_read`, `board_posts`, `board_post --text`, `board_delete --post-id`, `board_clear`

## Use `.lcity/agent_id`

```powershell
New-Item -ItemType Directory -Force .lcity | Out-Null
Set-Content .lcity\agent_id "eddy_lin"
node .\lcity\bin\lcity.mjs health_check
```

## Install locally as command

```powershell
npm --prefix .\lcity install
npm --prefix .\lcity link
lcity health_check
```

Optional base URL override:

```powershell
node .\lcity\bin\lcity.mjs --api-base http://localhost:3001 health_check
```

Output is always JSON:

```json
{"ok":true,"status_code":200,"data":{"status":"ok","agent_id":"eddy_lin","letta_agent_id":"...","current_location_id":"lin_bedroom","state":"idle"}}
```

All commands are designed for tool-calling and return machine-readable JSON.
