# lcity CLI (Node.js)

Shared command-line tool layer for all Letta agents in `letta-city-sim`.

This package is structured to be published independently.

- entrypoint: `bin/lcity.mjs`
- command implementation: `src/cli.mjs`
- package metadata: `package.json`

## Run directly

```powershell
$env:SIM_API_KEY="<your-sim-key>"
$env:LCITY_API_BASE="http://localhost:3001" # optional override
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
- `lettabot_notify --message "<text>" [--agent-id <id>]`

## Use `.lcity/agent_id`

```powershell
$env:SIM_API_KEY="<your-sim-key>"
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

SIM API key options:

```powershell
# env var
$env:SIM_API_KEY="devkey"

# or one-off CLI flag
node .\lcity\bin\lcity.mjs --sim-key devkey board_read
```

Output is always JSON:

```json
{"ok":true,"status_code":200,"data":{"status":"ok","agent_id":"eddy_lin","letta_agent_id":"...","current_location_id":"lin_bedroom","state":"idle"}}
```

All commands are designed for tool-calling and return machine-readable JSON.

## Notify LettaBot from the CLI

The CLI daemon can forward simple text messages to a LettaBot agent via the `/v1/chat/completions` endpoint (default base `http://127.0.0.1:8080`).

1. Export LettaBot credentials and start the daemon:

```powershell
$env:SIM_API_KEY="devkey"
$env:LETTABOT_API_KEY="user-api-key"
$env:LETTABOT_BASE="http://127.0.0.1:8080" # optional, defaults to https://api.letta.com
lcity daemon --start --lettabot-base $env:LETTABOT_BASE
```

2. Send a notification once the daemon is running:

```powershell
lcity lettabot_notify --message "Task finished"

# override agent id if needed
lcity lettabot_notify --message "Broadcast" --agent-id sam_moore
```

Under the hood, the CLI posts to the local daemon (`/notify`), which reuses your `LETTABOT_API_KEY` + base URL to call `/v1/chat/completions`.
