# Adding jobs

This guide explains how to add or refine jobs in `letta-city-sim` without redesigning the simulation.

## The important distinction

There are now **two related but different concepts**:

- `agents.occupation` — the in-world label for an NPC's identity (`Cafe Owner`, `Professor`, `Artist`)
- `jobs` / `agent_jobs` — the reusable runtime job catalog and current job assignments

Do **not** replace or repurpose `occupation` when contributing new jobs.

Use the new jobs system for:

- town jobs (`shopkeeper`, `librarian`, `groundskeeper`)
- meta roles (`dispatcher`, `toolsmith`, `researcher`, `writer`, etc.)

## Job kinds

Each job has a `kind`:

- `town` — roles grounded in the Smallville world and its venues
- `meta` — organization / contributor / agent-team roles that can coordinate work above the town layer

## Files you will usually touch

- `seed/jobs.sql` — add or refine job definitions
- `seed/agent_jobs.sql` — assign seeded agents to jobs
- `docs/community-contributions.md` — update contributor-facing guidance if a new lane opens up
- `lcity/README.md` or other docs — if you add examples that depend on the new job

## Safe contribution patterns

Good community-safe job changes:

- add a new entry to `seed/jobs.sql`
- improve a job's `summary`
- refine the `metadata` fields (`typical_tasks`, `deliverables`, `interfaces_with`, `guardrails`, `contributor_notes`)
- add a new seeded assignment in `seed/agent_jobs.sql`
- add docs/examples for how a role should be used

## What stays maintainer-owned

Do **not** open broad PRs that redesign these without explicit maintainer direction:

- database/schema direction beyond additive job work
- automatic job scheduling / shift systems
- wages, payroll, or economy policy
- hidden orchestration layers that change wake/interrupt behavior
- full role-execution engines for agents

If a contribution needs those, it probably belongs in a maintainer-owned issue.

## Job metadata shape

The first-pass job system keeps the schema intentionally small. Most contributor-facing structure lives in `metadata`.

Recommended keys:

- `typical_tasks` — short list of normal work the role does
- `deliverables` — what the role tends to produce
- `interfaces_with` — other jobs/roles it often collaborates with
- `guardrails` — limits or important constraints
- `contributor_notes` — guidance for people extending the role

You do **not** need every key for every job, but keep entries consistent and grounded.

## Example: add a new town job

Add a row to `seed/jobs.sql`:

```sql
(
  'florist',
  'Florist',
  'town',
  'Maintains flower stock and helps townspeople choose bouquets and gifts.',
  '{"typical_tasks": ["arrange flowers", "help customers"], "interfaces_with": ["groundskeeper", "writer"], "guardrails": ["Keep the role grounded in existing venues unless new locations are added."], "contributor_notes": "Good candidate for a future market or garden-adjacent venue."}'::jsonb
)
```

If a seeded agent should hold it, add an assignment to `seed/agent_jobs.sql`:

```sql
('some_agent_id', 'florist', TRUE, 'Starter assignment for the flower shop.')
```

## Example: add a new meta role

Meta roles are for the broader agent/contributor organization layer.

Example:

```sql
(
  'editor',
  'Editor',
  'meta',
  'Improves clarity, structure, and tone across written outputs.',
  '{"typical_tasks": ["review drafts", "tighten wording", "catch inconsistencies"], "interfaces_with": ["writer", "inspector"], "guardrails": ["Improve clarity without changing approved meaning."], "contributor_notes": "Useful for docs, blog posts, and lore packs."}'::jsonb
)
```

## Testing your job changes

From the repo root:

```powershell
docker compose up db -d
powershell -ExecutionPolicy Bypass -File .\scripts\seed.ps1

cd world-api
$env:DATABASE_URL="postgres://sim:sim_dev_password@localhost:5432/letta_city_sim"
cargo check
```

Then smoke-test the job routes/commands:

```powershell
node .\lcity\bin\lcity.mjs list_jobs
node .\lcity\bin\lcity.mjs get_job --id dispatcher
node .\lcity\bin\lcity.mjs list_agent_jobs --agent-id eddy_lin
```

## Quick rule of thumb

If your change is mostly **new role content, metadata, assignments, or docs**, it is probably community-safe.

If your change is mostly **new orchestration logic, policy, or schema redesign**, stop and check with a maintainer first.
