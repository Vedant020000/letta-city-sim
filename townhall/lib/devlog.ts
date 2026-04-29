export type DevLogEntry = {
  id: string;
  date: string;
  category: "backend" | "frontend" | "community" | "infrastructure";
  title: string;
  summary: string;
  bullets?: string[];
  links?: Array<{
    label: string;
    href: string;
  }>;
};

export const devLogEntries: DevLogEntry[] = [
  {
    id: "2026-04-29-bundled-docker",
    date: "2026-04-29",
    category: "infrastructure",
    title: "Optional bundled Docker image landed",
    summary:
      "There is now an optional single-image deployment/demo path that packages the frontend and world-api together while still keeping Postgres separate.",
    bullets: [
      "Added a bundled Dockerfile and compose path",
      "Kept the normal local workflow unchanged",
      "Made the frontend proxy /api and /ws/events internally in bundled mode",
      "Documented the one-port deployment path for demos and distribution",
    ],
  },
  {
    id: "2026-04-29-sleep-interaction",
    date: "2026-04-29",
    category: "backend",
    title: "First sleep interaction shipped",
    summary:
      "Agents can now enter and exit sleep using real bed objects. The first implementation is deliberately room-level and simple, which is exactly what the project needed right now.",
    bullets: [
      "Added sleep and wake routes in the World API",
      "Seeded the first real bed object for Eddy's bedroom",
      "Tracked bed occupancy in world object state",
      "Added CLI commands and testing coverage for the flow",
    ],
  },
  {
    id: "2026-04-29-townhall-status",
    date: "2026-04-29",
    category: "community",
    title: "Townhall grew into an actual project surface",
    summary:
      "Townhall is no longer just a pile of issues. It now has a project status page, shared navigation, and the beginnings of a proper public narrative for the sim.",
    bullets: [
      "Added a dedicated /status page",
      "Separated the issue board from the broader project snapshot",
      "Created space for public progress updates and dev logs",
    ],
    links: [
      { label: "Open board", href: "/" },
    ],
  },
  {
    id: "2026-04-29-townhall-pages",
    date: "2026-04-29",
    category: "infrastructure",
    title: "Townhall became GitHub Pages-ready",
    summary:
      "The community board was converted to static export + client-side GitHub fetching so it can live publicly on GitHub Pages without needing a running server.",
    bullets: [
      "Switched the board to browser-side GitHub issue loading",
      "Added a Pages deployment workflow",
      "Configured the repo for workflow-based GitHub Pages publishing",
    ],
  },
  {
    id: "2026-04-29-frontend-foundation",
    date: "2026-04-29",
    category: "frontend",
    title: "The first frontend engine finally exists",
    summary:
      "There is now a real maintainer-owned frontend foundation instead of vague frontend aspirations floating in the docs.",
    bullets: [
      "Next.js + Phaser frontend scaffolded in frontend/",
      "REST bootstrap wired for agents, locations, and world time",
      "Live /ws/events subscription added",
      "Placeholder map, agent markers, and raw event feed shipped",
    ],
  },
  {
    id: "2026-04-29-backlog-and-guides",
    date: "2026-04-29",
    category: "community",
    title: "Community backlog and contributor guides got real structure",
    summary:
      "The project now has a much better public ramp: contribution docs, practical guides, and a backlog that is sequenced instead of chaotic.",
    bullets: [
      "Added CONTRIBUTING.md and practical guides under docs/guides/",
      "Reworked frontend issues around the actual engine foundation",
      "Restructured backend/content issues into dependency-aware waves",
    ],
    links: [
      { label: "Contributing guide", href: "https://github.com/Vedant020000/letta-city-sim/blob/main/CONTRIBUTING.md" },
    ],
  },
  {
    id: "2026-04-29-interrupt-pipeline",
    date: "2026-04-29",
    category: "backend",
    title: "Interrupt handling was centralized",
    summary:
      "Manual messages and daemon-driven wakes now flow through the same interrupt abstraction instead of scattering wake logic everywhere.",
    bullets: [
      "Unified world-event and manual interrupts behind one pipeline",
      "Made the daemon and CLI use the same transport abstraction",
      "Documented the interrupt path so future changes have a clean home",
    ],
  },
];
