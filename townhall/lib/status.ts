export const projectStatus = {
  title: "Current city-sim status",
  summary:
    "letta-city-sim is now past the pure-planning stage: the World API is real, the interrupt pipeline is centralized, the first frontend engine exists, Townhall is live, and the first sleep interaction has landed.",
  overallStatus: "World foundation live; moving toward first real autonomous agent runs.",
  nextFocus: "Stamina hooks and first-agent bootstrap.",
};

export const statusSnapshot = [
  {
    title: "World API",
    value: "Substantially complete",
    description: "Agents, movement, locations, pathfinding, inventory, economy, vitals scaffold, board, objects, events, and websocket stream are live.",
  },
  {
    title: "CLI + daemon",
    value: "Live",
    description: "lcity command surface exists, interrupt handling is centralized, and daemon/manual wake paths now share the same pipeline.",
  },
  {
    title: "Frontend engine",
    value: "MVP foundation live",
    description: "Next.js + Phaser frontend now boots from the World API, listens to /ws/events, and renders placeholder town state and markers.",
  },
  {
    title: "Sleep interaction",
    value: "First pass shipped",
    description: "Agents can now sleep and wake via real bed objects using room-level occupancy rules, with CLI support and event logging.",
  },
  {
    title: "Townhall",
    value: "Public and deployable",
    description: "Community issue board is live, Pages-ready, and now has the foundation for a project status/dev-log surface.",
  },
  {
    title: "Community backlog",
    value: "Structured",
    description: "Frontend, backend, content, docs, art, and playtesting issues are now sequenced more realistically instead of being a flat pile.",
  },
];

export const builtSoFar = [
  "Rust/Axum World API with PostgreSQL-backed world state",
  "Agents, locations, nearby lookups, and Dijkstra pathfinding",
  "Inventory transfers, stackable consumables, economy, and vitals scaffold",
  "Notice board, world objects, append-only events, and websocket event stream",
  "Centralized interrupt pipeline for daemon-driven and manual Letta wakeups",
  "Maintainer-owned frontend engine with Next.js + Phaser + raw event feed",
  "First real sleep interaction using seeded bed objects and occupancy state",
];

export const roadmapColumns = [
  {
    title: "Now",
    items: [
      "Stabilize the core world interactions now that sleep is in place.",
      "Keep Townhall and contributor docs usable for outside contributors.",
      "Polish the frontend/status surfaces enough to make progress legible.",
    ],
  },
  {
    title: "Next",
    items: [
      "Add placeholder stamina hooks tied to movement and sleep.",
      "Bootstrap the first real autonomous agent run on top of the current stack.",
      "Use the frontend + event feed to observe and debug live agent behavior.",
    ],
  },
  {
    title: "Later",
    items: [
      "Multi-agent proving with conversations and richer town activity.",
      "Better map visuals, sprite work, movement polish, and interaction UI.",
      "Public dev-log/update thread below this status surface.",
    ],
  },
];

export const contributionSplit = [
  {
    title: "Maintainer-owned right now",
    items: [
      "Core architecture and world-model decisions",
      "Wake / interrupt internals and lifecycle semantics",
      "Auth, schema direction, and other architecture-sensitive systems",
      "First-agent bootstrap and other foundational proving work",
    ],
  },
  {
    title: "Community-open now",
    items: [
      "Frontend polish and panels on top of the new engine",
      "Location/item/content expansion packs",
      "Docs, guides, art, assets, and playtesting",
      "Bounded backend seed-data additions and quality-of-life improvements",
    ],
  },
];
