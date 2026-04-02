Now that we have the inner backend mechanics solid, here is your complete outer layer tech stack and modular infrastructure for lettagetns. This monorepo design leverages your Turborepo expertise for seamless scaling across web, daemon, and shared packages. [perplexity](https://www.perplexity.ai/search/e97b97dd-1e3b-42c1-9e63-b82821ce8c58)

## Complete Tech Stack
1. **Monorepo Orchestration:** Turborepo + pnpm for caching and parallel builds.
2. **Frontend:** Next.js 15 (App Router) + Phaser 3 + Grid Engine for Stardew style rendering.
3. **Backend State:** Convex (replaces SQLite) for ECS, realtime subscriptions.
4. **Agent Runtime:** Letta Cloud (GLM-4.7/Claude) + local daemon for dev.
5. **Auth:** Clerk for invite only human access + agent API keys.
6. **Tools:** TypeScript (Convex) + Python (Letta tool proxies).
7. **Deployment:** Railway (monorepo) + Convex Cloud + Letta Cloud.
8. **Worldgen:** Procedural TS scripts using Perlin noise.
9. **Docs:** MDX in /docs with VitePress for agent role catalog.

## Modular Folder Structure
Designed for atomic additions: new role = new YAML + tool file. Deploy with `turbo run build --filter=web...`. [github](https://github.com/joonspk-research/generative_agents)

```
lettagetns/
├── apps/
│   ├── web/                    # Production frontend
│   │   ├── app/                # Next App Router pages [page.tsx][layout.tsx]
│   │   ├── components/
│   │   │   ├── ui/             # Shadcn buttons, modals [button.tsx]
│   │   │   └── game/           # Phaser wrappers [CityCanvas.tsx][AgentSprite.tsx]
│   │   ├── pages/api/tools/    # HTTP gateways [cook.ts][[...proxy].ts]
│   │   ├── game/               # Phaser core [phaserConfig.ts][scenes/CityScene.ts]
│   │   └── public/sprites/     # Pixel art assets [chef.png]
│   └── letta-daemon/           # Letta runtime [daemon.ts]
│       └── src/
│           ├── agents/         # Registration [registerRoles.ts]
│           └── tools/          # Python proxies [cook.py]
├── packages/
│   ├── agents/                 # Modular roles/tools
│   │   ├── roles/              # YAML configs [chef.yaml][dispatcher.yaml]
│   │   └── tools/              # Shared logic [pathfind.ts][cook.ts]
│   ├── ecs/                    # Simulation engine
│   │   ├── utils/              # Queries [queryEntities.ts]
│   │   └── processors/         # Cron logic [movement.ts]
│   └── worldgen/               # Map scripts [generateCity.ts]
├── convex/                     # Backend mutations
│   ├── schema.ts
│   ├── tools/                  # Actions [cook.ts]
│   └── processors/             # Scheduled [movement.ts]
├── docs/                       # Self documenting [architecture.md][agent-roles.md]
├── turbo.json                  # Build pipelines
└── package.json                # Root deps: @convex-dev/*, letta-client
```

## Agent Files Modularity
Every agent lives as a self contained YAML + tool pair. Daemon watches /packages/agents for changes.

**Example `packages/agents/roles/chef.yaml`:**
```yaml
name: sim-chef-1
persona: "Efficient city chef prioritising resident hunger"
llm: "glm-4.7"
tools: ["cook", "pathfind", "gather"]
jobs:
  cook: "Monitor hunger events, claim free stove, execute recipe"
memory:
  recipes: "pasta: flour+water; biryani: rice+chicken+spices"
```

Daemon auto creates Letta agent: `client.agents.create(config)` + registers tools.

## Documentation Strategy
1. **architecture.md:** Mermaid diagrams of ECS flow.
2. **agent-roles.md:** Table of all 15 roles with YAML snippets.
3. **tools-catalog.md:** Auto generated from /tools/ folder via script.
4. **deployment.md:** Railway + Convex env vars checklist.

## Infra Deployment Flow
```
1. turbo run worldgen → Seed Convex
2. turbo run letta-daemon → Register agents
3. turbo run build --filter=web → Deploy Railway
4. Invite users via Clerk dashboard
```

This outer structure ensures you can add Therapist role in 5 minutes: YAML + Python proxy + Convex mutation. Perfect for your NIOS schedule while scaling to 100 agents.

Which part should we code first: Convex schema or Letta daemon startup?