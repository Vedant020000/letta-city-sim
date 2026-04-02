# letta-city-sim — Product Requirements Document

**Version:** 1.0
**Author:** Vedant Sondur
**Date:** April 2026
**Status:** Active Planning

---

## Overview

`letta-city-sim` is an open-source city simulation engine where every inhabitant is a **live, autonomous Letta agent running in real time**. There is no central game loop, no tick, no turn order. Each agent wakes up, reasons about the world, calls the World API, acts, and schedules its next decision entirely on its own clock — exactly like a person living their life.

The **World API** is a Rust/Axum REST service backed by PostgreSQL. It is the single source of truth for everything physical: where agents are, what they are doing, what they carry, what the environment looks like, and what conversations are happening. Agents read from and write to it via plain HTTP tool calls registered in their Letta thread.

Letta handles all agent cognition and memory. The World API handles all physical state. The frontend (Next.js + Phaser 3) visualises the live simulation on a 2D tile map.

---

## Core Design Principle

> **Every agent is an autonomous process. The world is a shared PostgreSQL database exposed over REST. Agents coordinate only through the world — never directly.**

Agent A cooking lunch, Agent B asleep, Agent C mid-conversation with Agents D and E — all happening simultaneously, all on independent real-time cadences. No orchestrator. No turn queue.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  Next.js 15 Frontend                                            │
│  ┌───────────────────────────┐  ┌────────────────────────────┐  │
│  │  Phaser 3 Canvas          │  │  React UI Panels           │  │
│  │  · Tiled town map         │  │  · Agent Inspector         │  │
│  │  · Agent sprites + walk   │  │  · Conversation Log        │  │
│  │  · Speech bubbles         │  │  · Event Feed              │  │
│  │  · Click-to-inspect       │  │  · Inventory               │  │
│  │  · Camera pan / zoom      │  │  · Simulation Controls     │  │
│  └───────────────────────────┘  └────────────────────────────┘  │
│                    ↕ EventEmitter bridge                         │
└──────────────────────────┬──────────────────────────────────────┘
                           │ REST + WebSocket
┌──────────────────────────▼──────────────────────────────────────┐
│  World API  (Rust / Axum)                                       │
│  /agents  /locations  /pathfind  /inventory                     │
│  /conversations  /objects  /events  /ws/events                  │
└──────────────────────────┬──────────────────────────────────────┘
                           │ sqlx / PostgreSQL 16
┌──────────────────────────▼──────────────────────────────────────┐
│  PostgreSQL 16                                                  │
│  agents · locations · location_adjacency · world_objects        │
│  inventory_items · conversations · conversation_participants    │
│  conversation_messages · events                                 │
└─────────────────────────────────────────────────────────────────┘
         ↑ webhooks          ↑ HTTP tool calls
┌────────┴────────┐  ┌───────┴────────────────────────────────────┐
│  Letta Cloud    │  │  Letta Agents  (one per NPC)               │
│  (memory,       │  │  · Eddy Lin                                │
│   cognition,    │  │  · Isabella Rodriguez                      │
│   scheduling)   │  │  · Klaus Mueller                           │
└─────────────────┘  │  · Maria Lopez  · Sam Moore  · Abigail Chen│
                     └────────────────────────────────────────────┘
```

---

## Tech Stack

| Layer | Technology | Why |
|---|---|---|
| World API | Rust + Axum | Fearless concurrency, compile-time SQL verification, perfect for high-throughput concurrent agent calls |
| Async runtime | Tokio | Native Axum/sqlx integration |
| Database driver | sqlx | Async, query macros verify SQL at compile time, no heavy ORM overhead |
| Database | PostgreSQL 16 | Full local control, JSONB for flexible object state, row-level locking for inventory transfers |
| JSON | serde / serde_json | De-facto standard in Rust |
| HTTP middleware | tower-http | CORS, tracing, compression |
| Agent runtime | Letta Cloud | Persistent memory, tool calling, self-scheduling via delayed messages |
| Frontend framework | Next.js 15 (App Router) | Already familiar, excellent DX |
| Map rendering | Phaser 3 | Full game framework — handles tilemaps, sprite animation, input, camera natively |
| UI panels | React (inside Next.js) | Inspector, event feed, conversation log |
| Phaser ↔ React | EventEmitter bridge | Clean separation — Phaser fires events, React listens and updates panels |
| Auth | Clerk | Control panel gating |
| Local dev | Docker Compose | One command to boot Postgres + World API + Frontend |

---

## World API

### Endpoints

```
AGENTS
  GET    /agents                          list all agents + current state
  GET    /agents/:id                      single agent (location, activity, inventory)
  PATCH  /agents/:id/location             update agent's current location
  PATCH  /agents/:id/activity             set current activity
  DELETE /agents/:id/activity             clear current activity (mark complete)
  GET    /agents/:id/nearby               agents and objects within same location

LOCATIONS
  GET    /locations                       list all locations
  GET    /locations/:id                   detail (occupants, objects, description)
  GET    /locations/:id/nearby            adjacent reachable locations

PATHFINDING
  GET    /pathfind?from=:a&to=:b          path array + travel_time_seconds (BFS over adjacency graph)

INVENTORY
  GET    /inventory/:agentId              items the agent is carrying
  POST   /inventory/:agentId/add          add item to agent's inventory
  POST   /inventory/:agentId/remove       remove item
  POST   /inventory/transfer              atomic transfer between two agents or agent ↔ location

WORLD OBJECTS
  GET    /objects/:locationId             objects present at a location
  PATCH  /objects/:id                     update object state (e.g. stove → { "on": true })

CONVERSATIONS
  POST   /conversations                   initiate a new conversation (first speaker)
  GET    /conversations/:id               state, messages, active participants
  POST   /conversations/:id/join          agent joins an existing conversation
  POST   /conversations/:id/leave         agent exits gracefully
  POST   /conversations/:id/message       post a message (broadcast to all participants)
  GET    /conversations/active?locationId live conversations at a location

EVENT LOG
  GET    /events?since=:iso&locationId=:id  filtered event log (append-only, never mutated)
  POST   /events                            append a custom event

WORLD
  GET    /world/time                      current real-world timestamp + time-of-day label

WEBSOCKET
  WS     /ws/events                       real-time event stream (frontend + agent webhooks)
```

All mutations require `X-Sim-Key` header. All responses are JSON.

### Concurrency Rules

- Location and activity updates: last-write-wins (fine for simulation scale).
- Inventory transfers: atomic — validate both sides exist before committing, wrapped in a Postgres transaction.
- Event log: append-only, no updates, no deletes.
- Pathfinding: stateless, any number of parallel callers safe.
- Conversations: `conversation_participants` uses `(conversation_id, agent_id)` primary key — duplicate joins are a no-op.

---

## Group Conversations

Conversations are first-class entities, not just message pairs. Any number of agents at the same location can participate simultaneously.

### Flow

1. **Agent A initiates** — calls `speak_to(agentB, message)` → `POST /conversations` → conversation created, A and B are participants, message stored, World API webhooks Agent B's Letta thread.
2. **Agent B responds** — their Letta thread fires, they call `reply(conversationId, message)` → `POST /conversations/:id/message` → all participants notified.
3. **Agent C arrives** — `look_around()` returns `activeConversations` at this location. C decides to join → `POST /conversations/:id/join`. All further messages reach C too.
4. **Natural exit** — when an agent's reasoning decides they are done, they call `leave_conversation(id)`. Conversation closes when participant count hits zero.

### What this enables

- **Coffee shop group chat** — Eddy and Isabella talking; Klaus walks in and joins mid-conversation.
- **Town meetings** — An agent posts to the notice board; multiple agents see it via `observe_events` and converge at the park. All join the same conversation. Emergent town meeting.
- **Passive eavesdropping** — An agent at the same location can see the conversation exists via `look_around()` without joining it. They can observe the topic without participating.
- **Overheard snippets** — The event log records every message. Agents at the same location can `observe_events` and see what was said even if they were not a participant.


### Webhook Routing (World API → Letta)

The World API calls `POST /v1/agents/:id/messages` (Letta's native endpoint) whenever an event should wake an idle agent. This is the only integration point between the World API and Letta.

| Trigger | Who gets woken |
|---|---|
| `speak_to(agentId, message)` | Target agent |
| `conversations/:id/message` posted | All current participants except sender |
| `conversations/:id/join` | All existing participants |
| Agent enters a location with an active conversation | That agent (optional, configurable) |
| Notice posted to a board | All agents at that location |
| `interact_with` on a shared object | Any agent whose activity references that object |

The message payload sent to Letta is a natural-language description of the event — e.g. `"Isabella just said: 'Has anyone seen Eddy today?' in the Hobbs Cafe conversation."` The agent reads it, reasons, and decides whether to reply, ignore, or act.

---

## Agent Tools

These are the Letta tool functions registered to every NPC agent. Each is a thin HTTP wrapper.

| Tool | Description | API call |
|---|---|---|
| `look_around()` | Current location detail — occupants, objects, active conversations | `GET /locations/:currentLocationId` |
| `move_to(locationId)` | Travel to a new location; returns path and ETA | `GET /pathfind` → `PATCH /agents/:id/location` |
| `set_activity(description)` | Declare what you are doing right now | `PATCH /agents/:id/activity` |
| `finish_activity()` | Mark current activity as complete | `DELETE /agents/:id/activity` |
| `check_inventory()` | What am I carrying | `GET /inventory/:id` |
| `pick_up(objectId)` | Take item from current location | `POST /inventory/:id/add` + `PATCH /objects/:id` |
| `put_down(objectId)` | Leave item at current location | `POST /inventory/:id/remove` |
| `give_to(agentId, objectId)` | Hand item to another agent | `POST /inventory/transfer` |
| `speak_to(agentId, message)` | Initiate a conversation | `POST /conversations` |
| `reply(conversationId, message)` | Post to an active conversation | `POST /conversations/:id/message` |
| `join_conversation(conversationId)` | Join a nearby conversation | `POST /conversations/:id/join` |
| `leave_conversation(conversationId)` | Exit a conversation | `POST /conversations/:id/leave` |
| `interact_with(objectId, action)` | Use a world object | `PATCH /objects/:id` |
| `observe_events(since)` | Read recent happenings near me | `GET /events?since=:ts&locationId=:id` |
| `check_world_time()` | What time of day is it | `GET /world/time` |

### Agent Lifecycle: Event-Driven Shutdown

When an agent completes any task — sleeping, cooking, working, reading — it simply goes idle. No polling, no running loop, zero compute cost. This applies to every long-running activity, not just sleep.

The agent wakes up in exactly one of two ways:

1. **Self-interrupt (Letta-side)** — the agent configures a Letta scheduled/delayed message before going idle ("wake me in 8 hours"). This is entirely handled by Letta's own scheduling config, not our code.
2. **External interrupt (World API-side)** — when something relevant happens to an idle agent (someone speaks to them, enters their location, posts a relevant notice), the World API calls Letta's `POST /v1/agents/:id/messages` endpoint with the event as the message content. The agent wakes, reads the context, and decides how to react.

This means our code only needs to know one thing: **which events should wake which agents**. The World API maintains a simple mapping — when a `conversation.message` event fires, notify all active participants; when an agent `speak_to`s another, notify the target; when a notice is posted at a location, notify all agents currently at that location. Everything else is Letta's problem.

---

## PostgreSQL Schema

```sql
CREATE TABLE locations (
  id           TEXT PRIMARY KEY,
  name         TEXT NOT NULL,
  description  TEXT NOT NULL,
  map_x        INTEGER NOT NULL,
  map_y        INTEGER NOT NULL,
  created_at   TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE location_adjacency (
  from_id      TEXT REFERENCES locations(id),
  to_id        TEXT REFERENCES locations(id),
  travel_secs  INTEGER NOT NULL,
  PRIMARY KEY  (from_id, to_id)
);

CREATE TABLE agents (
  id                  TEXT PRIMARY KEY,
  name                TEXT NOT NULL,
  occupation          TEXT NOT NULL,
  current_location_id TEXT REFERENCES locations(id),
  current_activity    TEXT,
  activity_started_at TIMESTAMPTZ,
  is_asleep           BOOLEAN DEFAULT FALSE,
  letta_webhook_url   TEXT NOT NULL,
  created_at          TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE world_objects (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  location_id TEXT REFERENCES locations(id),
  state       JSONB NOT NULL DEFAULT '{}',
  actions     TEXT[] NOT NULL DEFAULT '{}',
  created_at  TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE inventory_items (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  held_by     TEXT REFERENCES agents(id),
  location_id TEXT REFERENCES locations(id),
  state       JSONB NOT NULL DEFAULT '{}',
  created_at  TIMESTAMPTZ DEFAULT NOW(),
  CONSTRAINT held_xor_located CHECK (
    (held_by IS NULL) != (location_id IS NULL)
  )
);

CREATE TABLE conversations (
  id          TEXT PRIMARY KEY,
  location_id TEXT REFERENCES locations(id),
  topic       TEXT,
  started_at  TIMESTAMPTZ DEFAULT NOW(),
  ended_at    TIMESTAMPTZ
);

CREATE TABLE conversation_participants (
  conversation_id TEXT REFERENCES conversations(id),
  agent_id        TEXT REFERENCES agents(id),
  joined_at       TIMESTAMPTZ DEFAULT NOW(),
  left_at         TIMESTAMPTZ,
  PRIMARY KEY (conversation_id, agent_id)
);

CREATE TABLE conversation_messages (
  id              TEXT PRIMARY KEY,
  conversation_id TEXT REFERENCES conversations(id),
  agent_id        TEXT REFERENCES agents(id),
  content         TEXT NOT NULL,
  sent_at         TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE events (
  id          BIGSERIAL PRIMARY KEY,
  occurred_at TIMESTAMPTZ DEFAULT NOW(),
  type        TEXT NOT NULL,
  actor_id    TEXT REFERENCES agents(id),
  location_id TEXT REFERENCES locations(id),
  description TEXT NOT NULL,
  metadata    JSONB NOT NULL DEFAULT '{}'
);

CREATE INDEX idx_events_location ON events(location_id, occurred_at DESC);
CREATE INDEX idx_events_actor    ON events(actor_id, occurred_at DESC);
CREATE INDEX idx_agents_location ON agents(current_location_id);
CREATE INDEX idx_conv_active     ON conversations(location_id) WHERE ended_at IS NULL;
CREATE INDEX idx_inventory_held  ON inventory_items(held_by);
```

---

## Repo Structure

```
letta-city-sim/
├── world-api/                    ← Rust / Axum
│   ├── src/
│   │   ├── main.rs
│   │   ├── routes/
│   │   │   ├── agents.rs
│   │   │   ├── locations.rs
│   │   │   ├── pathfind.rs
│   │   │   ├── inventory.rs
│   │   │   ├── conversations.rs
│   │   │   ├── objects.rs
│   │   │   ├── events.rs
│   │   │   └── ws.rs             ← WebSocket handler
│   │   ├── models.rs             ← Rust structs (serde + sqlx FromRow)
│   │   ├── db.rs                 ← PgPool setup
│   │   └── webhook.rs            ← fires Letta agent webhooks on conversation events
│   ├── migrations/               ← .sql files run by sqlx-cli
│   └── Cargo.toml
│
├── frontend/                     ← Next.js 15
│   ├── app/
│   ├── components/
│   │   ├── PhaserMap.tsx         ← mounts Phaser into a React div
│   │   ├── AgentInspector.tsx
│   │   ├── ConversationLog.tsx
│   │   ├── EventFeed.tsx
│   │   └── ControlPanel.tsx
│   └── game/
│       ├── scenes/
│       │   ├── TownScene.ts      ← tilemap, agent sprites, click handling
│       │   └── UIScene.ts        ← speech bubbles, floating labels
│       ├── bridge.ts             ← EventEmitter: Phaser ↔ React
│       └── ws-client.ts          ← WebSocket client → pushes to bridge
│
├── seed/                         ← JSON seed data for locations, agents, objects
│   ├── locations.json
│   ├── agents.json
│   └── objects.json
│
├── docker-compose.yml
├── .env.example
└── README.md
```

---

## The Town: Smallville (v0.1 Seed)

```
The Ville
├── Residential Area
│   ├── Lin Family House      (bedroom · kitchen · living room)
│   ├── Morales Family House  (bedroom · kitchen · living room)
│   └── 4× Generic Houses     (bedroom · kitchen)
├── Town Centre
│   ├── Hobbs Cafe            (counter · seating area · kitchen)
│   ├── Harvey Oak Supply     (shop floor · storage room)
│   └── Ville Park            (east bench · west bench · fountain · notice board)
└── Oak Hill College
    ├── Classroom A
    ├── Staff Office
    └── College Cafe
```

**Starter cast (6 agents):**

| Agent | Occupation | Personality | Home |
|---|---|---|---|
| Eddy Lin | Music student | Practices obsessively, late nights | Lin Family House |
| Isabella Rodriguez | Cafe owner | Runs Hobbs Cafe, social butterfly | Morales Family House |
| Klaus Mueller | Professor | Long hours, coffee-dependent | near Oak Hill |
| Maria Lopez | Artist | Paints in the park, erratic schedule | Morales Family House |
| Sam Moore | Shop assistant | Works Harvey Oak Supply, very punctual | Own house |
| Abigail Chen | Student | Attends Oak Hill, hangs out with Eddy | Own house |

---

## Frontend Views

**Town Map** — The primary view. Agent avatars walk around the Tiled map in real time, positions driven by the World API WebSocket. A small floating label shows what each agent is currently doing. Click any agent to open the Inspector.

**Agent Inspector Panel** — Right-side drawer: current location, current activity, inventory list, recent events involving this agent, active conversations, and a debug input to inject a manual message into their Letta thread.

**Conversation Log** — When a conversation is active, clicking it shows the full message thread in real time, who joined/left, and the current topic.

**Event Feed** — Scrolling real-time log of all `SimEvent` entries. Filterable by agent, location, or event type.

**Control Panel** — Pause/resume all agents, inject a world event ("heavy rain begins at the park"), add a new agent with a custom persona, export world state as JSON.

---

## Docker Compose (Local Dev)

```yaml
services:
  db:
    image: postgres:16-alpine
    environment:
      POSTGRES_DB: letta_city_sim
      POSTGRES_USER: sim
      POSTGRES_PASSWORD: sim_dev_password
    ports:
      - "5432:5432"
    volumes:
      - pgdata:/var/lib/postgresql/data

  world-api:
    build: ./world-api
    environment:
      DATABASE_URL: postgres://sim:sim_dev_password@db:5432/letta_city_sim
      SIM_API_KEY: dev_key_change_me
    ports:
      - "3001:3001"
    depends_on: [db]

  frontend:
    build: ./frontend
    ports:
      - "3000:3000"
    environment:
      NEXT_PUBLIC_API_URL: http://localhost:3001
      NEXT_PUBLIC_WS_URL: ws://localhost:3001/ws/events

volumes:
  pgdata:
```

---

## Milestones

### Phase 1 — World API (Weeks 1–2)
- [ ] Rust / Axum project setup, Cargo.toml dependencies
- [ ] sqlx migrations for full schema
- [ ] All REST endpoints implemented and tested
- [ ] Concurrent write safety verified (inventory transfer transaction)
- [ ] WebSocket `/ws/events` broadcasting all mutations
- [ ] Seed script: loads Smallville locations, objects, 6 agent stubs
- [ ] Docker Compose boots everything with one command

### Phase 2 — First Agent (Week 3)
- [ ] Bootstrap Eddy Lin as a Letta agent with all tools registered
- [ ] Eddy can look around, move, set/clear activity, pick up objects
- [ ] Run Eddy unattended for 4 real hours — review event log for believability
- [ ] Webhook delivery verified (Letta notified when someone speaks to Eddy)
- [ ] Fix any tool call failures or reasoning loops

### Phase 3 — All 6 Agents + Conversations (Weeks 4–5)
- [ ] Spin up all 6 agents simultaneously
- [ ] Group conversation flow working end to end (initiate → join → reply → leave)
- [ ] Passive eavesdropping via event log verified
- [ ] Profile LLM cost: target under ₹300/day for 6 agents at normal activity
- [ ] At least one emergent group conversation observed without manual intervention

### Phase 4 — Frontend (Weeks 6–9)
- [ ] Tiled.js map in Phaser 3, embedded in Next.js
- [ ] Agent sprites moving with smooth interpolation
- [ ] Speech bubbles on active conversations
- [ ] Agent Inspector Panel (live polling World API)
- [ ] Conversation Log (real-time via WebSocket)
- [ ] Event Feed
- [ ] Control Panel (pause, inject event, add agent)

### Phase 5 — Open Source Release (Week 10)
- [ ] README with architecture diagram + quickstart
- [ ] Developer docs: adding a location, adding an agent, writing a new tool
- [ ] GitHub release as `letta-city-sim`
- [ ] Cognis blog post: architecture walkthrough + emergent behaviour observations

---

## Open Questions & Brainstorm

### Technical
1. **Model backend?** Claude Sonnet (quality) vs Gemini 2.5 Flash (cost/speed) — benchmark in Phase 2 on one agent before committing.
2. **Pause/resume mechanics** — pausing means the World API stops firing `message.create` interrupts. Letta-side scheduled messages continue ticking but the agents will find a paused world when they call any tool. A `simulation_paused` flag on `/world/time` is enough — agents read it and re-idle themselves.
3. **Notice board as a shared object** — agents can `interact_with(noticeBoardId, "post")` to leave a text notice. Other agents read it via `interact_with(noticeBoardId, "read")`. This is the cleanest emergent communication channel for large-group coordination.
4. **Agent death / new agent onboarding** — should agents have a finite lifespan or is the city eternal?

### Emergent Behaviour to Watch For
- A rumour seeded in one agent spreading through conversation chains to the rest of the town
- Multiple agents independently deciding to visit Hobbs Cafe in the morning → spontaneous social cluster
- An agent posting to the notice board about an event → others show up without being directly invited
- Eddy practising piano at 2 AM while everyone else is asleep — the city feeling genuinely alive at all hours

### Possible Future Features
- **Human player mode** — you inhabit one agent, your messages go into the simulation
- **Economy layer** — Harvey Oak Supply has stock, agents have currency, Sam actually sells things
- **Weather system** — time-of-day and weather metadata from `/world/time` affects agent mood and plans
- **Persistent multi-session world** — the city keeps running even when the browser is closed; you come back to find things have changed
- **Public observatory** — read-only hosted demo anyone can watch live
- **Agent creator UI** — drag-and-drop persona builder to add new citizens
- **Scenario injector** — predefined drama events ("the cafe runs out of coffee", "it starts raining", "there's a noise complaint about Eddy's piano")

