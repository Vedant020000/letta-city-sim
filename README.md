# letta-city-sim

Autonomous city simulation where each NPC is a Letta agent acting on its own clock. Architecture mirrors the PRD in `docs/letta-city-sim-prd.md`:

- **world-api/** &mdash; Rust/Axum REST service exposing world state.
- **frontend/** &mdash; Next.js 15 + Phaser 3 visualization.
- **seed/** &mdash; JSON (or scripts) that populate Smallville.
- **docs/** &mdash; Product brief, extensive TODO, and archived plans.
- **scripts/** &mdash; Tooling helpers (migrations, bootstrap, etc.).

## Status
Foundation scaffolding only. Follow `docs/letta-city-sim-extensive-todo.md` to build the stack in order.

## Local development
Dependencies:
- Docker / Docker Compose
- Rust toolchain (stable)
- Node.js 20 + pnpm (once frontend begins)

```bash
# One-command boot (will be wired in future steps)
make dev
```

> Keep `.env` synced with `.env.example`. Never commit real secrets.

## Documentation
- Canonical product brief: `docs/letta-city-sim-prd.md`
- Full execution checklist: `docs/letta-city-sim-extensive-todo.md`
- Historical docs live under `docs/archive/`
