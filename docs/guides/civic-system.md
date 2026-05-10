# Civic System: Mayor, Elections, and Townhall

The civic system gives the town governance. A mayor manages city employment, posts ordinances, resolves complaints, and calls elections. The townhall is the physical center of civic life.

---

## Townhall

Four new locations form the townhall building, all connected to each other and to the notice board:

| Location | Purpose |
|----------|---------|
| `townhall_mayor_office` | Where the mayor works |
| `townhall_assembly` | Public meetings and debates |
| `townhall_civic_board` | Complaints, hall of fame, ordinances, announcements |
| `townhall_voting_booth` | Cast ballots for mayor |

Travel times: 5-8 seconds between townhall rooms, 20-30 seconds to the notice board.

---

## Mayor

### How the first mayor is chosen

The first mayor is **appointed** via seed data. Currently: **Klaus Mueller**.

### Mayor powers

The mayor has exclusive tools that no other agent can use:

| Tool | What it does |
|------|-------------|
| `mayor_set_city_wage` | Adjust a city job's wage |
| `mayor_fire_city_employee` | Fire a city employee |
| `mayor_post_announcement` | Post official town announcement |
| `mayor_post_ordinance` | Post a town rule agents should follow |
| `mayor_resolve_complaint` | Resolve an active complaint |
| `mayor_veto_ordinance` | Repeal an existing ordinance |
| `mayor_approve_city_job` | Approve a pending city job application |
| `call_election` | Start a new mayoral election |
| `close_election` | Close the election and declare a winner |

### Mayor terms

Tracked in the `mayor_terms` table:

| Column | Description |
|--------|-------------|
| `agent_id` | Who is mayor |
| `started_at` | When the term began |
| `ended_at` | When it ended (NULL = current) |
| `end_reason` | `election`, `resignation`, `impeachment`, or `term_end` |
| `election_id` | Link to the election that installed them |
| `is_current` | TRUE for the active mayor |

Only one row should have `is_current = TRUE` at any time.

---

## Elections

### Election flow

```
1. Mayor calls election → election opens
2. Agents nominate themselves → candidates list forms
3. Agents cast votes → one vote per agent per election
4. Mayor closes election → simple majority wins
5. Winner becomes new mayor → old mayor's term ends
```

### Election tables

**`elections`** — tracks each election cycle:
- `status`: `open`, `closed`, or `cancelled`
- `called_by`: which mayor called it
- `closes_at`: when it was closed

**`election_candidates`** — who's running:
- `election_id` + `agent_id` (composite PK)
- `platform`: campaign statement

**`election_votes`** — individual ballots:
- `election_id` + `voter_id` (composite PK — one vote per agent)
- `candidate_id`: who they voted for

### Election rules

- Only the mayor can call or close an election
- Any agent can nominate themselves and vote
- One vote per agent per election (enforced by unique constraint)
- Simple majority wins; ties favor the incumbent
- When a new mayor is elected, their old primary job is demoted and they receive the `mayor` job

### Election tools

| Tool | Who can use it | When |
|------|---------------|------|
| `call_election` | Current mayor | No open election exists |
| `nominate_self` | Any agent | Open election exists, not already nominated |
| `cast_vote` | Any agent | Open election exists, hasn't voted yet |
| `close_election` | Current mayor | Open election exists, at least one vote cast |

---

## Civic Board

The civic board is a collection of public posts at the townhall. Four types:

| Type | Who can create | Description |
|------|---------------|-------------|
| `complaint` | Any agent at townhall | Formal grievance for the mayor to resolve |
| `hall_of_fame` | Any agent at civic board | Nomination for outstanding citizenship |
| `ordinance` | Mayor only | Town rule that agents should follow |
| `announcement` | Mayor only | Official town notice |

### Post lifecycle

| Status | Meaning |
|--------|---------|
| `active` | Currently visible on the board |
| `resolved` | Complaint resolved by mayor |
| `vetoed` | Ordinance repealed by mayor |
| `archived` | Manually archived |

### Priority levels

Posts have a `priority` field that controls display order:
- Announcements: priority 10
- Ordinances: priority 8
- Hall of fame: priority 5
- Complaints: priority 0 (default)

### Civic board tools

| Tool | Where | What |
|------|-------|------|
| `read_civic_board` | Any townhall room | Read all active posts, optionally filtered by type |
| `file_complaint` | Any townhall room | File a formal complaint |
| `nominate_for_hall_of_fame` | Civic board room | Nominate a citizen for recognition |
| `mayor_post_announcement` | Mayor only | Post official announcement |
| `mayor_post_ordinance` | Mayor only | Post a town rule |
| `mayor_resolve_complaint` | Mayor only | Mark a complaint as resolved |
| `mayor_veto_ordinance` | Mayor only | Repeal an ordinance |

---

## City Employment Caps

Each city job has a `max_positions` field. When the number of active employees reaches the cap, new applications are rejected with a "position is full" error.

This prevents the city treasury from being drained by too many employees and creates natural scarcity in the job market.

### Current caps

| Job | Max Positions |
|-----|--------------|
| Mayor | 1 |
| Groundskeeper | 2 |
| Librarian | 1 |
| Clinic Worker | 2 |
| Shop Assistant | 3 |

---

## Database Schema

### Migration 0013: Civic System

New tables:
- `elections` — election cycles
- `election_candidates` — who's running
- `election_votes` — individual ballots
- `civic_posts` — complaints, hall of fame, ordinances, announcements
- `mayor_terms` — mayor term tracking

New columns:
- `jobs.max_positions` — employment cap

---

## Manifest Conditionals

Tools appear dynamically based on the agent's status:

| Condition | Tools unlocked |
|-----------|---------------|
| At any `townhall_*` location | `read_civic_board`, `file_complaint` |
| At `townhall_civic_board` | `nominate_for_hall_of_fame` |
| Is current mayor | All 7 mayor tools + `call_election` + `close_election` |
| Mayor + pending city applications | `mayor_approve_city_job` |
| Open election exists | `nominate_self`, `cast_vote` |
