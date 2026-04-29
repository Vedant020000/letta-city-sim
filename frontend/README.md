# frontend

Minimum viable simulation frontend engine for `letta-city-sim`.

## Current MVP scope

- bootstrap agents, locations, and world time from the World API
- connect to `WS /ws/events`
- render a placeholder Phaser map using location `map_x` / `map_y`
- render simple agent markers
- show a raw websocket event feed for debugging

## Run locally

```powershell
cd frontend
npm install
npm run dev
```

Expected environment variables:

```powershell
$env:NEXT_PUBLIC_API_URL="http://localhost:3001"
$env:NEXT_PUBLIC_WS_URL="ws://localhost:3001/ws/events"
```

## Bundled Docker runtime

For the optional bundled image, the frontend runs behind a small custom Node server (`server.mjs`) and uses a single public port.

In that mode:
- browser API calls default to `/api`
- browser websocket connections default to `/ws/events`
- both are proxied internally to the bundled world-api

This bundled path is for demos/deployment convenience. Local development should still prefer the normal direct local API + websocket setup.
