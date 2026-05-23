# Event Routing Design

## Problem

The world emits ~15 event types (movement, board posts, sleep, jobs, conversations, etc.) but delivery is ad-hoc:
- **WS broadcast**: `event_tx().send()` fires to every connected WS client — no filtering.
- **Wake enqueue**: `enqueue_citizen_wake_tx()` is called manually in each action handler, with hardcoded target lists.
- **No routing policy**: There's no central place that decides *who should wake up for what*.

Result: agents either get woken for everything nearby (noisy, expensive) or miss events they should care about (because nobody wired the wake).

## Current Architecture

```
Action handler
  ├─ INSERT INTO events (...)           ← audit log, no routing
  ├─ event_tx().send(envelope)          ← WS broadcast, no filtering
  └─ enqueue_citizen_wake_tx(...)       ← manual per-action, hardcoded targets
       └─ citizen_signal_tx().send(id)   ← notify CLI/daemon
```

Every action handler independently decides who to wake. There's no shared routing logic.

## Proposed Architecture

```
Action handler
  ├─ INSERT INTO events (...)           ← audit log (unchanged)
  └─ route_event(event)                 ← single entry point
       ├─ evaluate routing rules
       ├─ for each matched agent:
       │    └─ enqueue_citizen_wake_tx(...)
       │         └─ citizen_signal_tx().send(id)
       └─ event_tx().send(envelope)     ← WS broadcast (unchanged, for frontend)
```

One function decides who wakes. Action handlers stop calling `enqueue_citizen_wake_tx` directly.

## Event Dimensions

Add to the `events` table:

| Field | Type | Purpose |
|-------|------|---------|
| `importance` | SMALLINT (1-5) | How significant is this event? 1=trivial, 5=critical |
| `visibility` | TEXT | Who can see it: `public`, `location`, `actor`, `target` |

These are set by the action handler when inserting the event, then read by the router.

## Routing Rules

The router evaluates rules in order. First match wins for each candidate agent.

### 1. Direct target (always wake)

Events that explicitly target an agent always wake them:
- `conversation.invite` → invited agent
- `job.application` → employer
- `money.request` → target agent
- `conversation.join_request` → conversation host

**Implementation**: `target_agent_ids` in event metadata. Router always wakes these agents.

### 2. Location-based (wake co-located agents)

Events at a location wake agents currently at that location, filtered by importance:

| Event importance | Who wakes |
|-----------------|-----------|
| 4-5 (high) | Everyone at the location |
| 2-3 (medium) | Agents with a role at the location (worker, resident, owner) |
| 1 (low) | Nobody (log only) |

**Examples**:
- Agent enters Hobbs Cafe (importance 2) → Isabella (shopkeeper) wakes, but not random passersby
- Agent starts a fight (importance 5) → everyone at the location wakes
- Agent picks up an item (importance 1) → nobody wakes

### 3. Role-based (wake agents whose occupation matches)

Some events are relevant to agents with specific roles, regardless of location:
- `board.post.created` about "music" → Eddy (musician)
- `election.opened` → all agents (civic duty)
- `bank.rate_changed` → banker

**Implementation**: keyword matching on event description/metadata against agent occupation and persona. Lightweight string matching, no LLM.

### 4. Relationship-based (future, not V1)

Wake agents with recent interactions with the actor. Deferred — requires relationship tracking that doesn't exist yet.

## Importance Levels

| Level | Label | Examples | Default routing |
|-------|-------|----------|----------------|
| 1 | trivial | item picked up, activity set | log only |
| 2 | routine | agent enters location, buys item | role-holders at location |
| 3 | notable | board post, job application, conversation start | co-located + role-matched |
| 4 | significant | election, bank rate change, money request | all at location + role-matched |
| 5 | critical | fire, arrest, death | everyone at location + adjacent |

## Visibility Levels

| Level | Who sees it | Examples |
|-------|-------------|----------|
| `public` | Everyone (WS broadcast + timeline) | Board posts, elections, shop openings |
| `location` | Agents at the same location | Conversations, sleep, item use |
| `actor` | Only the acting agent | Vitals check, balance check |
| `target` | Only the target agent | Money request, conversation invite |

## Implementation Plan

### Phase 1: Schema + Router (this issue)

1. **Migration**: Add `importance` (SMALLINT, default 2) and `visibility` (TEXT, default 'location') to `events` table.

2. **`route_event()` function**: New function in `routes/events.rs` that:
   - Takes an event (type, actor_id, location_id, importance, visibility, metadata)
   - Queries candidate agents based on routing rules
   - Calls `enqueue_citizen_wake_tx()` for each matched agent
   - Returns the list of woken agents

3. **Refactor action handlers**: Replace direct `enqueue_citizen_wake_tx()` calls with `route_event()`. This is the bulk of the work — ~15 call sites.

4. **WS broadcast unchanged**: `event_tx().send()` stays as-is for frontend consumption.

### Phase 2: Agent Interrupts Table (follow-up)

5. **`agent_interrupts` table**: Store routed wake decisions for observability and retry:
   ```
   id, agent_id, event_id, rule_matched, created_at, delivered_at
   ```
   This lets us debug "why did X wake up?" and "why didn't Y wake up?"

### Phase 3: Keyword Routing (follow-up)

6. **Keyword matching**: Add occupation/persona keywords to agent metadata. Router matches event descriptions against these keywords for role-based routing.

7. **Board post routing**: Board posts about "music" route to the musician, "gardening" to the gardener, etc.

## Anti-patterns to Avoid

- **Don't wake every agent for every event.** The whole point is selectivity.
- **Don't use LLMs for routing.** String matching is fast, cheap, and deterministic.
- **Don't make the router decide agent intentions.** Routing is about attention, not action.
- **Don't route in a background worker (yet).** Synchronous routing in the action handler is simpler and avoids race conditions. Async workers can come later if throughput demands it.

## Event Type Reference

Current event types and their proposed importance/visibility:

| Event Type | Importance | Visibility | Routing |
|-----------|-----------|------------|---------|
| `agent.moved` | 2 | location | co-located role-holders |
| `agent.sleep.started` | 2 | location | co-located agents |
| `agent.sleep.ended` | 2 | location | co-located agents |
| `board.post.created` | 3 | public | keyword-matched + co-located |
| `board.post.deleted` | 1 | actor | nobody |
| `conversation.started` | 3 | location | co-located agents |
| `conversation.invite` | 4 | target | invited agent (direct) |
| `conversation.join_request` | 4 | target | conversation host (direct) |
| `job.application` | 3 | target | employer (direct) |
| `job.hired` | 4 | target | applicant (direct) |
| `job.fired` | 5 | target | fired agent (direct) |
| `money.request` | 4 | target | target agent (direct) |
| `money.paid` | 3 | target | recipient (direct) |
| `bank.rate_changed` | 4 | public | banker + co-located |
| `election.opened` | 4 | public | all agents |
| `item.bought` | 1 | actor | nobody |
| `item.used` | 1 | actor | nobody |
| `agent.entered` | 2 | location | co-located role-holders |
