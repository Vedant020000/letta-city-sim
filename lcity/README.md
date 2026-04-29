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
- `sleep` — if the current location has exactly one usable bed, set the agent to sleeping and occupy that bed
- `wake_up` — exit sleep state and clear the occupied bed marker
- `list_inventory`
- `use_item --item-id <id> --quantity <n>` — consume stackable items, adjusts vitals
- `economy_update --amount-cents <n> [--reason "<text>"]` — credit (positive) or debit (negative) agent balance
- `board_read`, `board_posts`, `board_post --text`, `board_delete --post-id`, `board_clear`
- `current_intention`, `list_intentions`
- `set_intention --summary <text> --reason <text> [--expected-location-id <id>] [--expected-action <text>]`
- `update_intention --intention-id <id> [--summary <text>] [--reason <text>] [--expected-location-id <id>] [--expected-action <text>]`
- `complete_intention [--intention-id <id>] --outcome <text>`
- `fail_intention [--intention-id <id>] --outcome <text>`
- `abandon_intention [--intention-id <id>] --outcome <text>`
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

## Intention workflow

Agents should use intentions for meaningful multi-step action sequences:

```powershell
lcity current_intention
lcity set_intention --summary "Find old jazz sheet music" --reason "I want something new to practice tonight" --expected-location-id oak_classroom_a --expected-action research_archive
lcity move_to --location-id oak_classroom_a
lcity complete_intention --outcome "Found a promising lead in Klaus's old course packet."
```

`complete_intention`, `fail_intention`, and `abandon_intention` use the current active intention when `--intention-id` is omitted.

## Adding new commands

`src/cli.mjs` exposes a declarative `COMMANDS` registry. Each entry describes how the CLI should call the World API:

```js
const COMMANDS = {
  move_to: {
    route: "/agents/move",
    method: "PATCH",
    requiresAgent: true,
    buildBody: (options) => ({
      location_id: required(options, "location-id"),
    }),
  },
  // ...
};
```

- **route** – string or `(ctx, options) => string`. Use the helper to compute dynamic URLs.
- **method** – defaults to `GET`.
- **requiresAgent** – automatically injects `x-agent-id` header when true.
- **requireSimKey** – set to `false` for public endpoints (defaults to true).
- **buildBody(options, ctx)** – optional function to construct the JSON body from CLI flags.
- **handler(ctx, options)** – optional fully custom handler for advanced flows (`move_to_agent`, `daemon`, `lettabot_notify` use this).

The `run()` function now just looks up the command in the registry and passes the parsed CLI flags to the shared executor. To add a new command (e.g., `go_to_job` or `cook`), define a registry entry and update the usage list at the top of the README.

## Interrupt abstraction

The CLI now centralizes all wake/resume behavior behind a single internal function: `interruptAgent(...)`.

That means both of these flows share the same interrupt pipeline:

- daemon websocket events from the World API (`agent_targets` on `/ws/events`)
- manual `lettabot_notify` calls through the local daemon `/notify` endpoint

Each interrupt is normalized into a common shape with fields like:

- `agentId`
- `kind`
- `cause`
- `source`
- `message` or `payload`
- `transport`

Currently implemented transport:

- `lettabot_completion` — sends the interrupt to LettaBot via `/v1/chat/completions`

Reserved extension points for future work:

- `sdk`
- `webhook`

This keeps wake semantics in one place, so future adapters can be added without changing every daemon/manual call site.

## Stackable consumables

Inventory items now support `quantity`, `consumable_type`, and `vital_value` fields. When an agent uses a consumable:

- `use_item --item-id apple --quantity 1` decrements quantity and adjusts vitals
- If `consumable_type` is `food`, `water`, `stamina`, or `sleep`, the agent's corresponding vital is increased (clamped at 100)
- When quantity hits 0, the item row is deleted

Example:

```powershell
lcity use_item --item-id water_bottle --quantity 1
```

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

Under the hood, the CLI posts to the local daemon (`/notify`), which converts the request into a normalized interrupt and dispatches it through `interruptAgent(...)`. The current transport adapter then uses your `LETTABOT_API_KEY` + base URL to call `/v1/chat/completions`.
