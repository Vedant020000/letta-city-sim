# letta-city-sim — Extensive Todo

This is the full build order from zero to a releasable first version.

## Foundation

- [ ] Decide the monorepo structure: `world-api/`, `frontend/`, `seed/`, `docs/`, `scripts/`.
- [ ] Create the GitHub repo `letta-city-sim` and add a root `README.md`, `.gitignore`, `LICENSE`, `.env.example`, and `Makefile`.
- [ ] Add `docker-compose.yml` with Postgres, `world-api`, and `frontend` services so local development has one boot path.
- [ ] Define the canonical environment variables: `DATABASE_URL`, `SIM_API_KEY`, `LETTA_API_KEY`, `NEXT_PUBLIC_API_URL`, `NEXT_PUBLIC_WS_URL`, and Clerk keys if auth stays in scope.

## Database design

- [ ] Write the initial PostgreSQL migration for `locations`, `location_adjacency`, `agents`, `world_objects`, `inventory_items`, `conversations`, `conversation_participants`, `conversation_messages`, `events`, and `simulation_state`.
- [ ] Add indexes for event lookups, active conversations, agent-by-location queries, and inventory ownership queries so reads stay fast as the town gets busy.
- [ ] Use `jsonb` only where flexibility is genuinely needed, like `world_objects.state` and `events.metadata`, because PostgreSQL notes that JSON updates still lock the whole row and large JSON documents increase contention.[page:3]
- [ ] Keep inventory and conversation membership relational, not embedded JSON, so transfers and joins stay atomic and easy to query.[page:2][page:3]
- [ ] Seed the map: insert all locations, adjacency edges with `travel_secs`, starter objects, and the six starter agents.
- [ ] Install and standardise on `sqlx-cli` so migrations are reproducible and checked into the repo.

## Rust world-api scaffold

- [ ] Create the Rust service with Axum, Tokio, `sqlx`, `serde`, `tower-http`, `tracing`, and `thiserror`.
- [ ] Set up `main.rs` to load env vars, create the Postgres pool, initialise tracing, mount routes, and bind the HTTP server.
- [ ] Create shared `AppState` with the `PgPool` and the WebSocket broadcast channel.
- [ ] Add typed models for agents, locations, objects, conversations, messages, and events.
- [ ] Add a standard JSON error type so every failure path returns structured error responses.

## Authentication and safety

- [ ] Add `X-Sim-Key` middleware for all mutation routes so agents and admin panels cannot mutate the world anonymously.
- [ ] Add request logging and tracing IDs for every route because debugging concurrent agent behaviour without logs will be awful.
- [ ] Make every multi-row write path use transactions, especially inventory transfers, conversation creation, and any future shop/economy features.
- [ ] For conflicting writes on the same rows, use row-level locking like `SELECT ... FOR UPDATE` where needed, because PostgreSQL row locks block competing writers while allowing readers to continue.[page:2]
- [ ] Always acquire locks in a consistent order in transactional code to reduce deadlock risk, which PostgreSQL explicitly warns about.[page:2]

## Core routes

- [ ] Implement `GET /agents` and `GET /agents/:id` first so the frontend and debugging tools can inspect live state immediately.
- [ ] Implement `PATCH /agents/:id/location`, `PATCH /agents/:id/activity`, and `DELETE /agents/:id/activity` next, and append matching events for every mutation.
- [ ] Implement `GET /locations`, `GET /locations/:id`, and `GET /locations/:id/nearby` so agents can inspect the world around them.
- [ ] Implement `GET /pathfind?from=&to=` using BFS over `location_adjacency` and return both the path and total travel time.
- [ ] Implement inventory routes only after location and agent state are stable, because inventory correctness depends on reliable location state.
- [ ] Implement object routes so agents can read and mutate shared world state like beds, stoves, notice boards, and cafe machines.
- [ ] Implement conversation routes as a first-class system, not a hack on top of events, because synchronous multi-agent discussions are central to the product.

## Event log and realtime

- [ ] Make the `events` table append-only and treat it as the canonical history of what physically happened in the city.
- [ ] Implement filtered `GET /events` queries for debugging, observability, and the frontend feed.
- [ ] Add a WebSocket endpoint `/ws/events` and broadcast every mutation through a typed event envelope.
- [ ] Send a short backlog on WebSocket connect so the frontend can catch up without a blank screen.
- [ ] Keep the event payloads small and explicit because they will drive both the Phaser scene and the inspector panels.

## Webhook bridge to Letta

- [ ] Create a webhook module that sends natural-language event messages to Letta through the `message.create` path you are using.
- [ ] Look up the target agent’s Letta identifiers and webhook configuration from the database, not from hardcoded files.
- [ ] Fire these notifications asynchronously so API latency is not tied to Letta response times.
- [ ] Add retries with backoff and structured logs for failed deliveries.
- [ ] Implement the routing rules: direct speech wakes the target, conversation replies wake the other participants, joins notify the room, and notice board posts notify the relevant local agents.

## Letta tool layer

- [ ] Write the base system prompt template for all NPCs: persona, behaviour rules, tool usage expectations, shutdown behaviour, and how to interpret incoming interrupts.
- [ ] Create one tool wrapper per World API action: `look_around`, `move_to`, `set_activity`, `finish_activity`, `check_inventory`, `pick_up`, `put_down`, `give_to`, `speak_to`, `reply`, `join_conversation`, `leave_conversation`, `interact_with`, `observe_events`, and `check_world_time`.
- [ ] Standardise all tool responses so agents always receive predictable JSON shapes.
- [ ] Store the Letta agent IDs and message endpoint metadata in PostgreSQL so the world and the agents are linked cleanly.
- [ ] Build a bootstrap script that creates or syncs all starter agents from seed data.

## Single-agent proving

- [ ] Bring up just one agent first, ideally Eddy, with a minimal tool set: `look_around`, `move_to`, `set_activity`, and `check_world_time`.
- [ ] Verify that every action correctly mutates the database and emits a matching event.
- [ ] Confirm that the agent can go idle after choosing a long task and that no world-side loop keeps polling needlessly.
- [ ] Interrupt the idle agent using your Letta message flow and verify it wakes cleanly and resumes reasoning.
- [ ] Only after this passes should you enable inventory and conversation tools.

## Multi-agent proving

- [ ] Bring a second agent online and validate direct conversation initiation end to end: create conversation, store first message, notify target, receive reply, and persist it.
- [ ] Test a third agent joining an existing conversation at the same location, because group conversations are a core differentiator.
- [ ] Verify that leaving a conversation updates participant state correctly and that an empty conversation closes cleanly.
- [ ] Run all six agents together and inspect the event feed for duplicate wakeups, bad state transitions, and unbounded chatter.
- [ ] Measure real call frequency and cost patterns before you touch visual polish.

## Map and visual world

- [ ] Pick the visual style early: cosy 2D pixel art, clean palette, rounded UI, and readable sprites.
- [ ] Build the town map in Tiled and export the tilemap assets for Phaser.
- [ ] Assign `map_x` and `map_y` coordinates to locations and ensure the seed data and tilemap agree.
- [ ] Add sprite sheets for all starter agents with idle and walking states.
- [ ] Design notice boards, cafe furniture, beds, counters, and park props so the world reads instantly even at a glance.

## Next.js frontend scaffold

- [ ] Create the Next.js app with TypeScript and Tailwind.
- [ ] Use a client-only Phaser mount because Phaser does not run during SSR; the official Phaser/Next.js template explicitly uses a separated game structure and an event bus for React communication.[web:34][page:1]
- [ ] Create typed API and WebSocket clients for the World API so the UI and the game canvas share one contract.
- [ ] Add the React–Phaser event bridge from day one, because Phaser is best at the canvas while React is best at inspectors, logs, and controls.[page:1]

## Phaser experience

- [ ] Render the Tiled map and spawn all visible agents from `/agents` and `/locations` data.
- [ ] Animate movement smoothly when `move` events arrive over WebSocket.
- [ ] Add click-to-select on sprites and route the selection to the inspector panel.
- [ ] Show floating activity labels and short speech bubbles for active conversations.
- [ ] Add camera pan and zoom, depth sorting, and a subtle day/night overlay for polish.
- [ ] Keep Phaser focused on rendering and spatial interaction; keep forms, filters, logs, and management UI in React. The official Phaser/Next.js template recommends this split with game logic separated from UI and an event bus between them.[page:1]

## UI panels

- [ ] Build an Agent Inspector that shows location, activity, inventory, recent events, and active conversations.
- [ ] Build a Conversation Log that streams messages in real time and shows joins and leaves clearly.
- [ ] Build an Event Feed that can filter by agent, location, and event type.
- [ ] Build a Control Panel with pause/resume, manual event insertion, agent creation later if needed, and world export.
- [ ] Make the UI cute and extremely legible, because this product wins if it is delightful to watch.

## Testing and hardening

- [ ] Add integration tests for all critical API paths, especially pathfinding, transfers, and conversations.
- [ ] Add WebSocket tests so broadcast regressions are caught automatically.
- [ ] Test concurrent inventory transfers and conversation joins under load.
- [ ] Test long-running sessions where agents go idle and get reawakened many times over.
- [ ] Add a reset script that drops, migrates, and reseeds the world for fast iteration.

## Packaging and docs

- [ ] Write a real quickstart that gets a new contributor from clone to a running town with one command.
- [ ] Document the architecture: World API responsibilities, Letta responsibilities, the shutdown model, webhook routing, and the frontend split.
- [ ] Document how to add a new agent, a new location, and a new tool.
- [ ] Add diagrams for the data flow and the conversation lifecycle.
- [ ] Clean up secrets, tag a first release, and publish the repo once one small but complete vertical slice actually works.

## Recommended build order

- [ ] Finish Docker Compose and Postgres migrations before anything else.
- [ ] Then finish the Rust world-api scaffold and the agent/location/pathfinding routes.
- [ ] Then bootstrap one Letta agent and prove the shutdown plus interrupt model.
- [ ] Then add conversations and webhooks.
- [ ] Then add the WebSocket stream.
- [ ] Then build the Phaser map and React panels.
- [ ] Then expand to all six agents and polish the visuals.
