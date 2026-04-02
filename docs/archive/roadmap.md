# lettagetns Complete Development Roadmap

**Project Goal:** Invite-only generative agents city simulation where 15 specialised Letta agents live, work, and interact via ECS backend with beautiful Phaser frontend. MVP: 5 agents cooking, moving, and chatting in a 50x50 grid.

## Phase 0: Environment Setup (1 hour)
```
# Clone monorepo, install deps
pnpm create turbo@latest lettagetns
cd lettagetns
pnpm add -w @convex-dev/* clerk/nextjs phaser@3 grid-engine
pnpm add -w letta-client python  # For daemon
npx convex dev  # Local Convex dashboard
```
**Deliverable:** Empty Turborepo running, Convex dashboard accessible.

## Phase 1: Backend Foundation (2 hours)
```
# 1.1 Create ECS schema
convex/schema.ts  # Entities + components tables [code_file:1]

# 1.2 World generation script
packages/worldgen/generateCity.ts  # 50x50 grid, 5 buildings
npx convex run generateWorld  # Seed kitchen, homes

# 1.3 Core processors
convex/processors/movement.ts  # Cron every 500ms
convex/tools/perceive.ts  # Read 5x5 grid chunk
```
**Milestone:** Query `npx convex query listEntities` shows populated city.

## Phase 2: Tool Pipeline (3 hours)
```
# 2.1 Next.js API gateways
apps/web/pages/api/tools/[...proxy].ts  # Auth + Convex forwarder

# 2.2 Movement tools
packages/agents/tools/pathfind.ts  # EasyStar.js A*
convex/tools/moveTo.ts  # ECS position mutation

# 2.3 Cooking mechanics
convex/tools/cook.ts  # Inventory check + state change
packages/agents/tools/cook.py  # Letta HTTP proxy
```
**Milestone:** `curl POST /api/tools/cook` updates stove state in Convex.

## Phase 3: Letta Agent Integration (4 hours)
```
# 3.1 Daemon setup
apps/letta-daemon/daemon.ts  # Watches YAML, creates agents

# 3.2 Role configurations
packages/agents/roles/chef.yaml  # Persona + tools
packages/agents/roles/dispatcher.yaml  # Routes jobs

# 3.3 Registration script
pnpm turbo run registerAgents  # Letta client.agents.create()
```
**Milestone:** `ps aux | grep letta` shows daemon running 3 agents.

## Phase 4: Phaser Frontend (5 hours)
```
# 4.1 Game scene
apps/web/game/scenes/CityScene.ts  # Tilemap + GridEngine

# 4.2 Realtime sync
apps/web/components/game/CityCanvas.tsx  # ConvexReactClient.useQuery

# 4.3 Agent sprites
apps/web/public/sprites/  # Chef walking, cooking animations
AgentSprite.tsx  # State → animation mapping table
```
**Milestone:** Browser shows moving pixel agents on city map.

## Phase 5: Role System MVP (6 hours)
```
# 5.1 Implement 5 core roles
roles/chef.yaml, writer.yaml, dispatcher.yaml, debugger.yaml, therapist.yaml

# 5.2 Job delegation
Dispatcher tool calls other agents via /tools/delegate

# 5.3 Conversation system
convex/tools/speak.ts  # Bubble text + memory log
```
**Deliverable:** Chef cooks autonomously when hunger detected.

## Phase 6: Invite-Only Polish (2 hours)
```
# 6.1 Clerk integration
middleware.ts  # Protect /city page
apps/web/app/city/page.tsx  # Auth wall + live viewer

# 6.2 Rate limiting
convex/http.ts  # Per-agent quotas

# 6.3 Error boundaries
Debugger agent auto investigates failed tool calls
```
**Milestone:** Share Clerk invite link, observers watch live simulation.

## Phase 7: Production Deployment (1 hour)
```
# 7.1 Railway monorepo
railway up --filter web...  # Auto deploys turbo

# 7.2 Convex production
npx convex deploy

# 7.3 Letta Cloud migration
export LETTA_CLOUD_URL=...  # Daemon points to hosted agents
```
**Public Demo:** https://lettagetns.yourdomain.com (invite only).

## Phase 8: Scale to 15 Roles (Ongoing)
```
# Add remaining roles one per day:
therapist.yaml, architect.yaml, policy_officer.yaml, etc.

# Performance monitoring
Convex dashboard + Sentry for agent failures
```
**Final Goal:** Full city ecosystem with emergent behaviours.

## Success Metrics
- [ ] 50x50 grid rendered at 60fps
- [ ] 15 agents active, <5% tool failures
- [ ] 10 concurrent human observers
- [ ] Chef successfully delivers biryani to Writer agent

## Risk Mitigations
```
# Token costs: Use GLM-4.7 + batch 5s intervals
# Agent loops: 100ms Convex processor timeout
# Collisions: GridEngine handles agent path conflicts
# Asset art: Use free Stardew Valley style packs initially
```

**Total Timeline:** 24 hours to MVP. Perfect for your late night coding sessions. Start with Phase 1 schema now?