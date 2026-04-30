# Community contributions

This doc defines what goes to the community and what stays maintainer-owned in `letta-city-sim`.

For practical contributor onboarding, also see:

- `../CONTRIBUTING.md`
- `guides/README.md`
- `guides/adding-jobs.md`
- `guides/adding-locations.md`
- `guides/adding-items-and-consumables.md`
- `guides/playtesting.md`

## Maintainer-owned

These stay with project maintainers unless explicitly opened up:

- core architecture decisions
- wake / interrupt semantics
- auth / security model
- schema direction
- sleep / lifecycle rules
- other architecture-sensitive changes

These issues should be labeled with one of:

- `architecture-sensitive`
- `maintainer-only`

They should **not** be labeled `community`.

## Community contribution lanes

These are explicitly open for community help.

### 1. Backend / simulation features
- jobs
- restaurants
- foods
- items
- map locations
- isolated endpoints
- integration tests

Suggested labels:
- `community`
- `backend`
- `help wanted`

For job-related contributions specifically:

- use `seed/jobs.sql` for new job definitions
- use `seed/agent_jobs.sql` for seeded assignments
- keep broad scheduling/payroll/orchestration redesigns maintainer-owned
- prefer the guide in `docs/guides/adding-jobs.md` when opening or implementing job issues

### 2. Frontend
- town map UI
- inspector panels
- event feed
- control panel
- interaction polish

Suggested labels:
- `community`
- `frontend`

### 3. Playtesting
- run your own bots
- test long-running behavior
- compare models
- report loops / bad state transitions

Suggested labels:
- `community`
- `playtest`

### 4. Art / assets / worldbuilding
- sprites
- tile sets
- props
- NPCs
- locations
- lore / prompts

Suggested labels:
- `community`
- `art`
- `content`

### 5. Docs / onboarding
- quickstarts
- setup fixes
- command docs
- tutorial flows

Suggested labels:
- `community`
- `docs`
- `good first issue`

## Townhall board rules

The `townhall/` app reads GitHub issues and only shows issues that are meant for the community.

To appear on the board, an issue should usually have:

- `community`

and often one or more of:

- `good first issue`
- `help wanted`
- `backend`
- `frontend`
- `docs`
- `playtest`
- `art`
- `content`

## Claiming tasks

The board is intentionally auth-free.

Claim by commenting on the GitHub issue:

```text
/claim
```

Release with:

```text
/release
```

The board infers claim state from issue comments.

## Suggested initial public issue buckets

- add more jobs
- add more foods / consumables
- add more items
- add more locations
- add map art / props
- improve setup docs
- run playtests and report findings
- build frontend panels
