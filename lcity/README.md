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
