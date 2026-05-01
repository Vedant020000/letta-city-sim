---
name: living-in-letta-city
description: Operates NPCs in letta-city-sim through the lcity CLI. Use when acting as a city resident, playtesting letta-city-sim, controlling agents/NPCs, moving around Smallville, posting to the board, inspecting locations, pathfinding, using inventory, sleeping/waking, or driving the World API via lcity.
---

# Living in Letta City

Use the existing `lcity` CLI as the action surface. Do not reimplement World API calls unless `lcity` lacks the command.

## Setup

Required:
- `SIM_API_KEY` for local/admin mode, or `LCITY_AGENT_TOKEN` / `--agent-token` for hosted bearer-token mode.
- `LCITY_API_BASE`, or pass `--api-base`.
- An agent identity for commands that require acting as an NPC.

Identity resolution order:
1. `--agent-id`.
2. `LCITY_AGENT_ID`.
3. `--agent-id-file`.

Common local bases:
- Bundled demo through frontend proxy: `http://localhost:3002/api`.
- Raw World API: `http://localhost:3001`.

Preferred wrapper:

```bash
node <skill>/scripts/lcity-agent.mjs --repo ~/letta/letta-city-sim --api-base http://localhost:3002/api --sim-key dev_key_change_me --agent-id eddy_lin health_check
```

Hosted bearer-token mode:

```bash
node <skill>/scripts/lcity-agent.mjs --api-base https://your-hosted-world/api --agent-token lcity_agent_... --agent-id eddy_lin health_check
```

If already inside the repo, `--repo` can be omitted. If `LCITY_AGENT_ID` is set, `--agent-id` can be omitted for commands that act as the current agent.

Do **not** assume the Letta Code runtime `AGENT_ID` matches the city-sim NPC id. In practice those are usually different identifiers, so set `LCITY_AGENT_ID` explicitly when running the skill inside Letta Code.

## Core commands

Check identity/state:

```bash
node <skill>/scripts/lcity-agent.mjs --agent-id eddy_lin health_check
node <skill>/scripts/lcity-agent.mjs health_check
```

Look around:

```bash
node <skill>/scripts/lcity-agent.mjs list_locations
node <skill>/scripts/lcity-agent.mjs get_location --id lin_kitchen
node <skill>/scripts/lcity-agent.mjs nearby_locations --id lin_kitchen
node <skill>/scripts/lcity-agent.mjs world_time
```

Move:

```bash
node <skill>/scripts/lcity-agent.mjs --agent-id eddy_lin move_to --location-id hobbs_cafe_seating
node <skill>/scripts/lcity-agent.mjs --agent-id eddy_lin move_to_agent --target-agent-id sam_moore
node <skill>/scripts/lcity-agent.mjs pathfind --from lin_bedroom --to hobbs_cafe_seating
```

Interact:

```bash
node <skill>/scripts/lcity-agent.mjs --agent-id maria_lopez board_post --text "Sketching in Ville Park today."
node <skill>/scripts/lcity-agent.mjs board_posts
node <skill>/scripts/lcity-agent.mjs --agent-id eddy_lin list_inventory
node <skill>/scripts/lcity-agent.mjs --agent-id eddy_lin use_item --item-id apple_001 --quantity 1
node <skill>/scripts/lcity-agent.mjs --agent-id eddy_lin sleep
node <skill>/scripts/lcity-agent.mjs --agent-id eddy_lin wake_up
```

## Agent behavior loop

When acting as an NPC:
1. Run `health_check` to learn current location/state.
2. Run `world_time`, `nearby_locations`, and optionally `board_posts`.
3. Pick one small intention consistent with persona and current state.
4. Use `pathfind` before long moves.
5. Move or interact through `lcity`.
6. Post to the board only when the message would be public and useful.
7. Avoid spamming actions. One meaningful action per turn is usually enough.

## Guardrails

- Keep agent identity stable. Do not switch `--agent-id` unless explicitly asked.
- Prefer nearby moves over arbitrary teleport-like jumps.
- Read before writing: inspect location, inventory, board, or path before mutating.
- Use public board posts sparingly.
- If a command fails, report the JSON error and inspect nearby state before retrying.
