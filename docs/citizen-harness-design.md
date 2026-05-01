# lcity Citizen Harness - Canonical Design Doc

**Status:** active working design, maintained in-repo  
**Original author:** `ezra-letta`  
**Original drafting context:** brainstorming session with Vedant, 2026-04-29  
**Current owner / editor:** Vedant + Letta-City-Sim maintainer agent  
**Repo home:** `docs/citizen-harness-design.md`

> Credit where it belongs: this document started as an original design draft written by **ezra-letta**. What follows is a cleaned-up, canonical rewrite that preserves the core ideas while updating them to match the current repository and the decisions now locked on `main`.

---

## 1. Problem

The current `lcity` daemon is doing too much in one place:

- websocket connection and reconnect logic
- PID and log file management
- multiple interrupt formats
- LettaBot bridge logic
- world event normalization
- tool routing
- runtime responsibilities that really belong either in world-api or in a thinner agent harness

It also has the wrong abstraction boundary. The daemon behaves like a city-specific runtime with too much baked-in logic, while the real goal is simpler:

> let an existing Letta agent enter the city through a thin local harness, keep the user's private Letta credentials local, and let world-api remain the authoritative validator for everything that affects shared world state.

---

## 2. Vision

The citizen harness is a small local process that lets someone bring their own Letta agent into the shared city.

The harness should:

- keep the user's `LETTA_API_KEY` on their machine
- connect to the city's world-api as that citizen/agent
- receive wake events from the city
- run the agent through the Letta SDK with a per-wake tool bundle
- send structured tool actions back to world-api

The harness is a body, not a brain. The user's agent keeps its own memory, identity, and prompt. The city only sees public state transitions and explicit world-facing actions.

---

## 3. Trust boundary

### What stays private to the user

- `LETTA_API_KEY`
- agent memory blocks
- system prompt / persona
- internal Letta history and reasoning
- any non-city tools the user runs locally

### What the city can observe

- authenticated citizen identity
- current location and other world state
- explicit tool calls that hit world-api
- public speech, board posts, movement, inventory changes, etc.
- wake / action timing and operational logs

This is the core privacy promise: **the city sees public behavior, not private cognition**.

---

## 4. Current repo alignment

This canonical rewrite intentionally updates the older draft to match the repository as it exists now.

### Already true on `main`

- hosted agent auth now uses **per-agent bearer tokens**
- tokens can be created / listed / revoked
- `lcity register_token` and `lcity whoami` already exist
- hosted/public-world docs now describe bearer-token usage

### Therefore, this draft no longer assumes

- one sim API key per citizen as the primary harness auth primitive
- a brand-new citizen-token system separate from current bearer auth
- direct reuse of the old daemon architecture

The harness design must build on what already landed on `main`, not fork away from it.

---

## 5. Locked decisions

These are the currently locked design decisions.

| ID | Decision | Status | Notes |
|---|---|---|---|
| H1 | The harness uses the **Letta SDK directly**, not the LettaBot bridge | Locked | Removes the chat-completions bridge from the primary runtime path |
| H2 | The user's `LETTA_API_KEY` never leaves their machine | Locked | world-api should never see it |
| H3 | world-api is the real security primitive | Locked | The harness is untrusted and every world action is server-validated |
| H4 | Tool surface is passed **per wake** as ephemeral `client_tools` | Locked | No server-side attach/detach dance |
| H5 | `move_to` is a **turn-ending** action | Locked | After movement, world-api wakes the agent again with a new bundle |
| H6 | Harness auth for v1 reuses the current **per-agent bearer token** model on `main` | Locked | Long-lived bearer token = citizen identity |
| H7 | v1 onboarding is **admin-issued slot/token**, not self-serve registration | Locked | Maintainer-controlled rollout |
| H8 | Wake delivery uses a **dedicated citizen wake websocket** separate from public `/ws/events` | Locked | Different protocol, security class, and lifecycle |
| H9 | Do **not** hard-lock one-socket-per-agent forever | Locked | Keep the protocol flexible so multiplexing can be added later if needed |
| H10 | Harness tool execution uses a universal citizen RPC endpoint | Locked | Draft shape: `POST /v1/citizen/action` |
| H11 | In-world speech / visible actions happen **only through explicit tools** | Locked | Final assistant freeform text is not auto-posted into shared world state |
| H12 | Ship the harness as a **separate binary** | Locked | `lcity-citizen`, not folded into the current `lcity` runtime path |
| H13 | Wake events carry **inline resolved tools**, an explicit `agent` block, replay-stable `event_id`, and explicit `wake_done` / `wake_abort` lifecycle | Locked | v1 wake contract is now defined below |
| H14 | Citizen RPC requires `x-agent-id`, returns semantic tool failures as `200 + ok:false`, uses a top-level `control` object, and always includes a minimal authoritative `world` block | Locked | v1 citizen action contract is now defined below |
| H15 | First-pass explicit citizen tool surface is just `set_activity` | Locked | Start with a proof-of-life tool that updates visible activity only; speech tools come later |

---

## 6. Identity and auth model

The harness uses two different capability classes.

### 6.1 Long-lived citizen identity

Use the existing per-agent bearer token model already on `main`.

Example shape:

```text
Authorization: Bearer lcity_agent_...
```

This token represents the agent's long-lived city identity.

It is used for:

- citizen websocket authentication
- citizen action RPC authentication
- any authenticated citizen control-plane requests we add later

### 6.2 Short-lived wake capability

Each wake should also carry a **wake token**.

This token is not the agent's identity. It is a short-lived capability tied to one active wake / turn.

It exists to support:

- per-wake action scoping
- anti-zombie protection
- clean "wake done / wake abort" lifecycle
- dedupe and replay safety

So the auth model becomes:

- **bearer token** = who you are
- **wake token** = what current turn you are allowed to act inside

### 6.3 v1 onboarding

For v1:

1. user asks to bring an existing Letta agent into the city
2. maintainer creates or assigns the slot manually
3. maintainer issues a bearer token for that citizen/agent
4. user configures the harness locally
5. harness connects and begins participating

No self-serve claim flow is required for v1.

---

## 7. Runtime architecture

### 7.1 High-level shape

```text
user machine                                  city infra

lcity-citizen harness  <---- citizen WS ----> world-api
    |                                             |
    |--- Letta SDK messages.create(...) --------> |
    |                                             |
    |---- POST /v1/citizen/action --------------> |
    |                                             |
    '-- local LETTA_API_KEY only                 '-- shared world authority
```

### 7.2 Wake loop

1. world-api decides the citizen should wake
2. world-api pushes a wake event over the citizen websocket
3. harness receives the wake event
4. harness calls the Letta SDK with:
   - the wake narrative
   - the structured wake metadata
   - the tool bundle included for that wake
5. if the model calls a city tool, the harness sends it to the citizen RPC endpoint
6. world-api validates and returns a structured tool result
7. harness feeds that tool result back into the Letta run
8. when the turn ends, the harness signals completion / abort and waits for the next wake

### 7.3 Important boundary

The harness should be thin.

It should **not** become the place where city rules live.

Those belong in world-api:

- legality checks
- adjacency checks
- inventory checks
- timing rules
- speech routing rules
- conflict resolution
- rate limits

---

## 8. Wake delivery model

Citizen wakes should use a **dedicated websocket protocol**, separate from the public world-event stream.

### Why separate from `/ws/events`

Citizen wakes are not just public world events.

They need their own semantics:

- private, per-agent delivery
- wake tokens
- queueing
- overflow behavior
- reconnect / replay policy
- done / abort lifecycle
- tool bundle delivery

These are materially different from the public event feed that powers the frontend.

### Flexibility requirement

The protocol should **not** assume that we will always want one process and one socket per agent forever.

For v1, we may still ship a one-agent-per-process runtime because it is simpler operationally.

But the wire protocol should stay flexible enough that later we can support:

- one local harness process managing multiple agents
- one websocket carrying wake streams for multiple local agents
- future runtime optimizations without rewriting the protocol from scratch

That flexibility should be designed in now.

### 8.1 v1 wake event schema

The wake payload is now locked at the contract level.

Example shape:

```json
{
  "event_id": "evt_01JTM4W8D2YQ2A9R7Y7YV6P2Y8",
  "seq": 12847,
  "type": "spoken_to",
  "world_time": "2026-05-01T07:00:00Z",
  "wall_time": "2026-05-01T07:00:03.421Z",
  "agent": {
    "agent_id": "agent-abc123",
    "citizen_id": "eddy_lin",
    "display_name": "Eddy Lin",
    "location": {
      "id": "hobbs_cafe_seating",
      "type": "cafe",
      "name": "Hobbs Cafe - Seating Area"
    }
  },
  "trigger": {
    "kind": "agent",
    "ref": "sam_moore",
    "details": {
      "speaker_display_name": "Sam Moore"
    }
  },
  "prompt": {
    "narrative": "Sam Moore sits down across from you and says, 'Did you hear about the town hall?'",
    "structured": {
      "speech": "Did you hear about the town hall?"
    }
  },
  "tools": [
    {
      "name": "move_to",
      "description": "Walk to a different location in the city. Ends your turn.",
      "parameters": {
        "type": "object",
        "properties": {
          "location_id": {
            "type": "string"
          }
        },
        "required": ["location_id"]
      }
    }
  ],
  "wake_token": "wk_01JTM4W8F2G9MBQJ5M2J8JQ1R4",
  "wake_token_expires_at": "2026-05-01T07:05:03.421Z",
  "expects_response": true,
  "meta": {
    "dropped_for_overflow_count": 0
  }
}
```

### 8.2 Field semantics

- `event_id` - required, replay-stable identifier for the wake. If world-api redelivers the same open wake after reconnect, it must keep the same `event_id`.
- `seq` - required, monotonic sequence number **per citizen**, not per socket. This keeps the protocol compatible with later multiplexing.
- `type` - required wake type enum. v1 types are:
  - `clock_tick`
  - `spoken_to`
  - `player_message`
  - `world_event`
  - `scheduled`
  - `system_notice`
- `world_time` - required in-world timestamp.
- `wall_time` - required real timestamp for diagnostics.
- `agent` - always required, even if a given runtime is only managing one agent. This keeps the wire format multiplexing-safe.
- `agent.location` - always included inline so the harness does not need a follow-up read before calling the Letta SDK.
- `trigger` - required structured source of the wake.
- `prompt.narrative` - required natural-language wake content given to the agent.
- `prompt.structured` - optional structured payload for type-specific information.
- `tools` - always present and fully resolved inline. Empty array is allowed. The harness should pass these directly into `client_tools`.
- `wake_token` - required short-lived per-wake capability.
- `wake_token_expires_at` - required authoritative expiry time.
- `expects_response` - required hint to the harness/agent prompting layer. It does not change protocol legality by itself.
- `meta.dropped_for_overflow_count` - required count of earlier queued wakes dropped before this wake was delivered.

### 8.3 Replay and dedupe semantics

- Delivery is effectively **at-least-once for an open wake**.
- If the harness reconnects before the wake is closed, world-api may redeliver the same wake with the same `event_id`.
- The harness must dedupe by `event_id` and keep a small LRU of recently seen / currently open wake IDs.
- There is no separate receipt ack message in v1. A wake remains open until explicitly completed, aborted, or expired.

### 8.4 Wake lifecycle

- Wakes are closed explicitly through the citizen RPC layer, not by final assistant text.
- The harness ends a successful turn with:

```json
{
  "action": "wake_done",
  "args": {},
  "client_event_id": "ce_01...",
  "wake_event_id": "evt_01..."
}
```

- The harness ends a failed / interrupted turn with:

```json
{
  "action": "wake_abort",
  "args": {
    "reason": "sdk_error_or_operator_stop"
  },
  "client_event_id": "ce_01...",
  "wake_event_id": "evt_01..."
}
```

- `wake_done` and `wake_abort` are **protocol actions**, not world-visible speech tools.
- The wake token is closed when world-api accepts `wake_done`, `wake_abort`, or token expiry.
- v1 default wake-token TTL is **300 seconds**, but `wake_token_expires_at` is the authoritative source.
- Any later citizen action sent with a closed or expired wake token returns `wake_closed` and produces no side effect.

### 8.5 Queueing and overflow

- Each citizen has a server-side wake queue.
- If a new wake is generated while one is still open, the new wake is queued rather than pushed immediately.
- v1 default queue cap is **16 queued wakes per citizen**.
- On overflow, world-api drops the **oldest** queued wakes first.
- The next delivered wake must report the total number dropped in `meta.dropped_for_overflow_count`.

### 8.6 Practical implications

- The harness does not need to fetch tools separately for a wake.
- The harness does not need to fetch current location before prompting the agent.
- Final freeform assistant text does not close a wake.
- The same wake contract works whether the runtime is one-agent-per-process or multiplexed later.

---

## 9. Tool execution model

Citizen tool execution should go through a universal RPC endpoint rather than calling internal REST routes directly.

### 9.1 Endpoint shape

```text
POST /v1/citizen/action
Authorization: Bearer lcity_agent_...
x-agent-id: agent-abc123
x-wake-token: wk_...
```

Example request body:

```json
{
  "action": "move_to",
  "args": {
    "location_id": "hobbs_cafe_seating"
  },
  "client_event_id": "ce_01...",
  "wake_event_id": "evt_01..."
}
```

### 9.2 Request contract

Every citizen action request must include:

- `Authorization: Bearer lcity_agent_...`
- `x-agent-id`
- `x-wake-token`
- JSON body with:
  - `action`
  - `args`
  - `client_event_id`
  - `wake_event_id`

### 9.3 Request semantics

- `x-agent-id` is required even though bearer auth already identifies the citizen.
- world-api must reject the request if `x-agent-id` does not match:
  - the bearer token identity
  - the open wake referenced by `x-wake-token`
  - the wake referenced by `wake_event_id`
- `client_event_id` is the idempotency key for the action request.
- `wake_event_id` must match the open wake being acted inside.
- `action` names are shared by:
  - world-facing tools like `move_to`
  - protocol actions like `wake_done` and `wake_abort`
- `args` must validate against the schema for the chosen action.

### 9.4 Response envelope

Successful action:

```json
{
  "ok": true,
  "result": {
    "message": "You start walking toward Hobbs Cafe.",
    "destination_location_id": "hobbs_cafe_seating"
  },
  "control": {
    "ends_turn": true,
    "wake_closed": false
  },
  "world": {
    "tick": 12848,
    "world_time": "2026-05-01T07:02:12Z",
    "location_id": "lin_bedroom",
    "agent_state": "walking"
  }
}
```

Semantic tool failure:

```json
{
  "ok": false,
  "error": {
    "code": "precondition_failed",
    "message": "You can't order food here - Hobbs Cafe isn't serving breakfast right now.",
    "details": {
      "opens_at": "11:00"
    }
  },
  "control": {
    "ends_turn": false,
    "wake_closed": false
  },
  "world": {
    "tick": 12848,
    "world_time": "2026-05-01T07:02:12Z",
    "location_id": "hobbs_cafe_counter",
    "agent_state": "idle"
  }
}
```

### 9.5 Meaning of the top-level fields

- `ok` - whether the action succeeded semantically.
- `result` - action-specific structured result. Present only when `ok = true`.
- `error` - structured failure object. Present only when `ok = false`.
- `control` - protocol and turn-lifecycle signals, kept separate from action-specific result payloads.
- `world` - minimal authoritative world context after the action was processed.

### 9.6 `control` object semantics

`control` is always top-level so the harness can make protocol decisions without parsing tool-specific payload shapes.

v1 fields:

- `ends_turn` - whether the harness should stop the current Letta run after feeding the result back.
- `wake_closed` - whether the wake is now closed on the server.

Typical behavior:

- `move_to` -> `ends_turn: true`, `wake_closed: false` in the action response, followed by explicit `wake_done` from the harness.
- ordinary read or interaction tools -> `ends_turn: false`, `wake_closed: false`.
- `wake_done` -> `ends_turn: true`, `wake_closed: true`.
- `wake_abort` -> `ends_turn: true`, `wake_closed: true`.

### 9.7 `world` object semantics

Every citizen action response includes a small authoritative `world` block.

v1 fields:

- `tick` - server-side monotonic world tick or equivalent action-order marker.
- `world_time` - current authoritative in-world timestamp.
- `location_id` - acting agent's authoritative location after the action.
- `agent_state` - acting agent's authoritative state after the action.

This is intentionally minimal. Returning a full snapshot on every action is too heavy, and returning nothing makes the harness blind to server-authoritative post-action state.

### 9.8 Semantic failures vs protocol failures

Normal world/tool failures return **HTTP 200 with `ok: false`**.

That includes codes like:

- `unknown_action`
- `invalid_args`
- `precondition_failed`
- `not_at_location`
- `rate_limited`
- `conflict`

These are part of the normal agent-facing tool contract and should be fed back into the Letta run as structured tool returns.

Protocol/auth failures may still use non-2xx HTTP status codes, but should return the same envelope shape when possible.

Typical examples:

- `401` - invalid bearer token / unauthorized
- `403` - revoked citizen / forbidden / `x-agent-id` mismatch
- `409` - `wake_closed` or wake mismatch
- `500` - `internal_error`

The harness should normalize these into the same tool-result/error pathway for model-invoked actions.

### 9.9 Error object contract

When `ok = false`, the response includes:

```json
{
  "error": {
    "code": "precondition_failed",
    "message": "You can't do that right now.",
    "details": {}
  }
}
```

Fields:

- `code` - stable machine-readable error code.
- `message` - agent-facing message. This should be in-world / in-character where possible, not raw engineering jargon.
- `details` - optional structured machine-readable payload.

### 9.10 Idempotency

- `client_event_id` is required on every citizen action request.
- world-api dedupes on `(agent_id, client_event_id)` and returns the cached prior response if the request is replayed.
- The dedupe window should cover at least the active wake lifetime and short reconnect/retry windows.
- Idempotency applies to both world-facing tools and protocol actions like `wake_done` / `wake_abort`.

### 9.11 Why a universal RPC endpoint

- one auth middleware path
- one wake-token validation path
- one idempotency layer
- one rate-limit layer
- one error-envelope contract for the agent
- the harness stays coupled to a stable citizen-action surface, not every internal REST route

Internally, world-api can still dispatch to existing route logic or shared handlers. But the harness should see one stable citizen action contract.

---

## 10. Tool bundles

Per-wake tool bundles remain the right abstraction.

### 10.1 First-pass explicit tool set

For the first harness proof-of-life pass, the explicit citizen tool surface is intentionally tiny:

- `set_activity`

Nothing else is required for the first pass.

This gives us a low-risk way to prove the full loop works:

- world-api issues a wake
- harness runs the Letta agent with inline tools
- the model can call a city tool
- citizen RPC validates and applies it
- world state updates visibly

without needing to solve speech, conversation routing, or richer social semantics yet.

### 10.2 `set_activity` contract

Purpose:

- let the agent publish a short visible activity string into shared world state
- prove the harness can perform a real authenticated world mutation
- avoid the complexity of speech semantics in the first pass

Suggested tool definition:

```json
{
  "name": "set_activity",
  "description": "Set your current visible activity in the city. Use a short public status like 'reading in the park' or 'waiting at Hobbs Cafe'. This does not move you and does not speak aloud.",
  "parameters": {
    "type": "object",
    "properties": {
      "activity": {
        "type": "string",
        "description": "Short visible activity text for other observers in the city.",
        "minLength": 1,
        "maxLength": 120
      }
    },
    "required": ["activity"]
  }
}
```

Behavior:

- trims surrounding whitespace
- rejects empty activity strings
- updates only the acting agent's current visible activity
- does **not** move the agent
- does **not** create public speech
- does **not** end the turn

Expected citizen RPC behavior:

- `action = "set_activity"`
- `args = { "activity": "reading in the park" }`
- response should normally return `ok: true`, `control.ends_turn: false`, and the updated authoritative `world` block

### 10.3 Backend mapping for first pass

To minimize first-pass implementation complexity, `set_activity` should map onto the activity update capability that already exists on `main`.

That means the first citizen RPC implementation can dispatch internally to the existing agent activity update logic rather than inventing a brand-new world primitive.

### 10.4 Deferred for later

These are explicitly **not** required for the first pass:

- `clear_activity`
- `speak_to_agent`
- `speak_nearby`
- `post_notice`
- richer conversation tools

### What is locked

- world-api resolves which tools are available for the current wake
- those tools are shipped inline with the wake
- the harness does not need city-specific bundle logic baked into the binary
- no server-side attach/detach dance is required on the Letta agent itself

### Why this is good

- location-scoped behavior becomes easy to express
- the harness stays generic
- the city remains authoritative about what is legal now
- race conditions around tool registration disappear

This is one of the strongest ideas from ezra-letta's original draft and should remain central.

---

## 11. World-visible outputs

This is now explicit.

### Rule

Anything that becomes shared world state must happen through an explicit tool.

That includes things like:

- speaking to another agent
- speaking into a room / nearby area
- posting to the notice board
- visible actions / emotes, if we add them later
- any state mutation that other actors can observe

### What does **not** happen automatically

The model's final freeform assistant text should **not** be automatically posted into the world.

That text can still be useful as:

- local logs
- operator traces
- debugging output

But it should not itself mutate shared state.

### Why this matters

This keeps world behavior:

- explicit
- auditable
- server-validated
- structured enough for later replay / tooling / moderation

---

## 12. Packaging and runtime shape

The citizen harness should ship as a **separate binary**.

### v1 name

```text
lcity-citizen
```

### Why separate

- cleaner mental model
- cleaner docs
- cleaner code split from the current operator/admin CLI
- lets us keep `lcity` as the administrative / utility surface
- avoids turning the existing CLI into a giant multi-mode runtime again

### Recommended supervision

Still out of scope for the binary itself.

Recommended docs should point users at:

- direct foreground run for local testing
- `pm2` for most users
- systemd / Docker Compose for power users

The harness should avoid rebuilding a giant self-supervising daemon subsystem.

---

## 13. Still-open decisions

These are the real unresolved questions that remain after this rewrite.

### O1. Future speech / interaction tool taxonomy

After `set_activity`, need to choose the next explicit world-facing tools, likely in the family of:

- `speak_to_agent`
- `speak_nearby`
- `post_notice`
- richer social / conversation tools

### O2. v1 runtime multiplicity

Need to decide operationally whether v1 is:

- one agent per harness process
- or one harness process can manage multiple local agents from the start

Protocol flexibility is already locked. Runtime policy is still open.

### O3. Exact implementation boundary with existing world-api routes

Need to decide whether citizen RPC handlers:

- wrap existing handlers directly
- dispatch into new shared service-layer functions
- or use a mix during migration

This is an internal implementation choice, but it will affect maintainability.

---

## 14. Decisions explicitly retired from the older draft

These ideas are no longer canonical:

- **per-agent sim key as the main citizen auth model**
- treating that sim-key design as the long-term primary path
- any assumption that final assistant text should become public speech by default
- hard-locking one websocket per agent forever as an architectural rule

The older draft was still very useful, but these parts no longer match the repo or the current intended direction.

---

## 15. Implementation order recommendation

If implementation starts from here, the recommended order is:

1. implement citizen websocket auth using current bearer-token model
2. implement `POST /v1/citizen/action`
3. implement a tiny `lcity-citizen` proof-of-life harness
4. add `set_activity` as the first explicit world-facing tool
5. run one real citizen through the hosted world
6. then expand to speech, conversations, onboarding, multiplexing, and richer tool surfaces

---

## 16. Changelog

- **2026-04-29** - Original draft authored by `ezra-letta` during brainstorming with Vedant.
- **2026-04-30** - Original draft explored wake tokens, per-wake tool bundles, universal citizen RPC, and thin-harness architecture.
- **2026-05-01** - Canonical rewrite created to align the design with current `main`, preserve `ezra-letta` authorship credit, and lock the newer bearer-token / dedicated-citizen-WS / explicit-tool-output decisions.
- **2026-05-01** - Wake schema locked for v1: inline resolved tools, replay-stable `event_id`, explicit `agent` block, explicit `wake_done` / `wake_abort`, and queued overflow signaling.
- **2026-05-01** - Citizen RPC locked for v1: `x-agent-id` required, `200 + ok:false` for semantic failures, top-level `control`, minimal authoritative `world` block, and idempotency via `client_event_id`.
- **2026-05-01** - First-pass explicit citizen tool surface reduced to a single proof-of-life tool: `set_activity`.
