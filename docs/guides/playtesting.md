# Playtesting guide

This guide is for contributors who want to help by **running the sim**, not necessarily by writing code.

Playtesting is one of the highest-value community contributions right now.

## What makes a good playtest report

A good report includes:

- what you ran
- how long you ran it
- which model/backend you used if relevant
- what you expected
- what actually happened
- logs/screenshots/snippets if possible

Useful findings include:

- loops
- repeated wakeups
- silent agents that never seem to act
- strange location behavior
- odd event spam
- clearly missing tools/content

## Common playtest scenarios

### 1. Single-agent unattended run
Best for:

- repeated loops
- idle behavior
- wake/interrupt oddities

### 2. Two-agent same-location scenario
Best for:

- social behavior
- nearby presence
- shared-location weirdness

### 3. Multi-agent soak test
Best for:

- event spam
- wake storms
- model cost/stability observations

### 4. Model comparison
Best for:

- comparing behavior quality
- comparing stability/cost tradeoffs

## Basic local setup

```powershell
docker compose up db -d
powershell -ExecutionPolicy Bypass -File .\scripts\seed.ps1

cd world-api
$env:DATABASE_URL="postgres://sim:sim_dev_password@localhost:5432/letta_city_sim"
$env:SIM_API_KEY="dev_key_change_me"
cargo run
```

Then run whichever agent tooling flow is relevant for the scenario you are testing.

## What to capture

When possible, capture:

- timestamps
- agent ids/names involved
- location ids involved
- event types seen
- whether behavior repeated
- any error output

Even short notes are useful if they are concrete.

## Suggested report template

You can paste something like this into a GitHub issue comment:

```md
## Environment
- OS:
- Model/backend:
- Runtime length:
- Agents involved:

## Scenario
- What I tried:

## What I expected
-

## What happened
-

## Evidence
- Logs/screenshots/snippets:

## Follow-up ideas
-
```

## How to make reports actionable

Good:

- "Eddy Lin repeated the same move loop for 14 minutes between `lin_bedroom` and `lin_kitchen`."
- "The six-agent soak test produced repeated `location.enter` events for the same agent within seconds."

Less useful:

- "it felt weird"
- "something broke maybe"

## Related issues

Current playtesting issues already on the board include:

- single-agent unattended runs
- two-agent same-location tests
- six-agent soak tests
- model comparison tasks

If you discover a clear new bug, open a follow-up issue and link your playtest report.
