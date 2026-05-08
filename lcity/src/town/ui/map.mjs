const AGENT_COLORS = [
  "blue", "red", "green", "magenta", "yellow", "cyan",
  "white", "bright-blue", "bright-red", "bright-green",
];

function agentColor(agentId) {
  let hash = 0;
  for (const char of agentId) {
    hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  }
  return AGENT_COLORS[hash % AGENT_COLORS.length];
}

function getInitials(name) {
  return name
    .split(" ")
    .map((p) => p[0])
    .slice(0, 2)
    .join("")
    .toUpperCase();
}

function formatTime(iso) {
  if (!iso) return "-";
  const d = new Date(iso);
  return d.toLocaleTimeString("en-US", { hour12: false, hour: "2-digit", minute: "2-digit" });
}

function truncate(str, max) {
  if (!str) return "";
  return str.length > max ? str.slice(0, max - 1) + "…" : str;
}

export async function startTownMap({ apiBase, simKey, pollMs = 2000 } = {}) {
  const blessedModule = await import("blessed");
  const blessed = blessedModule.default ?? blessedModule;

  const screen = blessed.screen({
    smartCSR: true,
    fullUnicode: true,
    title: "letta city",
  });

  const header = blessed.box({
    parent: screen,
    top: 0,
    left: 0,
    width: "100%",
    height: 3,
    tags: true,
    style: { fg: "white", bg: "blue" },
    padding: { left: 1, right: 1 },
  });

  const mapBox = blessed.box({
    parent: screen,
    label: " {bold}Town Map{/bold} ",
    top: 3,
    left: 0,
    width: "75%",
    height: "100%-6",
    tags: true,
    border: { type: "line" },
    style: { border: { fg: "cyan" } },
    padding: { left: 1, right: 1, top: 1, bottom: 1 },
  });

  const sidebar = blessed.box({
    parent: screen,
    top: 3,
    left: "75%",
    width: "25%",
    height: "100%-6",
    tags: true,
    border: { type: "line" },
    style: { border: { fg: "cyan" } },
    padding: { left: 1, right: 1, top: 0, bottom: 0 },
    scrollable: true,
    alwaysScroll: true,
  });

  const footer = blessed.box({
    parent: screen,
    bottom: 0,
    left: 0,
    width: "100%",
    height: 3,
    tags: true,
    style: { fg: "black", bg: "white" },
    padding: { left: 1, right: 1 },
  });

  let state = {
    locations: [],
    agents: [],
    worldTime: null,
    events: [],
    selectedAgentId: null,
    error: null,
    lastUpdate: null,
  };

  function renderMap() {
    const { locations, agents } = state;
    if (locations.length === 0) {
      mapBox.setContent("{center}Loading map...{/center}");
      return;
    }

    const innerW = mapBox.width - 4;
    const innerH = mapBox.height - 4;

    const minX = Math.min(...locations.map((l) => l.map_x));
    const maxX = Math.max(...locations.map((l) => l.map_x));
    const minY = Math.min(...locations.map((l) => l.map_y));
    const maxY = Math.max(...locations.map((l) => l.map_y));

    const spreadX = Math.max(maxX - minX, 1);
    const spreadY = Math.max(maxY - minY, 1);

    const scaleX = Math.max(1, Math.floor((innerW - 20) / spreadX));
    const scaleY = Math.max(1, Math.floor((innerH - 8) / spreadY));
    const scale = Math.min(scaleX, scaleY, 3);

    const padX = Math.floor((innerW - spreadX * scale) / 2);
    const padY = Math.floor((innerH - spreadY * scale) / 2);

    const agentsByLoc = new Map();
    for (const agent of agents) {
      const list = agentsByLoc.get(agent.current_location_id) || [];
      list.push(agent);
      agentsByLoc.set(agent.current_location_id, list);
    }

    const locPositions = new Map();
    for (const loc of locations) {
      const x = padX + Math.floor((loc.map_x - minX) * scale);
      const y = padY + Math.floor((loc.map_y - minY) * scale);
      locPositions.set(loc.id, { x, y, loc });
    }

    const gridH = innerH;
    const gridW = innerW;
    const grid = [];
    for (let r = 0; r < gridH; r++) {
      grid[r] = Array(gridW).fill(" ");
    }

    function setCell(r, c, ch, color = "white") {
      if (r >= 0 && r < gridH && c >= 0 && c < gridW) {
        grid[r][c] = `{${color}-fg}${ch}{/}`;
      }
    }

    function drawBox(top, left, w, h, label, color) {
      for (let r = top; r < top + h && r < gridH; r++) {
        for (let c = left; c < left + w && c < gridW; c++) {
          if (r === top || r === top + h - 1) {
            setCell(r, c, "─", color);
          } else if (c === left || c === left + w - 1) {
            setCell(r, c, "│", color);
          } else {
            setCell(r, c, " ", color);
          }
        }
      }
      setCell(top, left, "┌", color);
      setCell(top, left + w - 1, "┐", color);
      setCell(top + h - 1, left, "└", color);
      setCell(top + h - 1, left + w - 1, "┘", color);

      const labelStart = left + Math.max(1, Math.floor((w - label.length) / 2));
      for (let i = 0; i < label.length && labelStart + i < left + w - 1; i++) {
        setCell(top, labelStart + i, label[i], color);
      }
    }

    function drawDot(r, c, color) {
      setCell(r, c, "●", color);
    }

    for (const [locId, { x, y, loc }] of locPositions) {
      const boxW = Math.max(12, loc.name.length + 4);
      const boxH = 5;
      const bx = Math.max(0, Math.min(x - Math.floor(boxW / 2), gridW - boxW - 1));
      const by = Math.max(0, Math.min(y - Math.floor(boxH / 2), gridH - boxH - 1));

      drawBox(by, bx, boxW, boxH, truncate(loc.name, boxW - 2), "bright-cyan");

      const locAgents = agentsByLoc.get(locId) || [];
      for (let i = 0; i < locAgents.length; i++) {
        const agent = locAgents[i];
        const ax = bx + 2 + (i % 3) * 2;
        const ay = by + boxH - 2 + Math.floor(i / 3);
        if (ax < bx + boxW - 1 && ay < gridH - 1) {
          const color = agentColor(agent.id);
          const selected = state.selectedAgentId === agent.id;
          const ch = selected ? "◉" : "●";
          drawDot(ay, ax, selected ? "bright-white" : color);
        }
      }
    }

    mapBox.setContent(grid.map((row) => row.join("")).join("\n"));
  }

  function renderSidebar() {
    const lines = [];

    if (state.worldTime) {
      lines.push(`{bold}Time:{/bold} ${formatTime(state.worldTime.timestamp)} ${state.worldTime.time_of_day || ""}`);
      if (state.worldTime.simulation_paused) {
        lines.push(`{red-fg}{bold}PAUSED{/bold}{/}`);
      }
      lines.push("");
    }

    lines.push(`{bold}Locations:{/bold} ${state.locations.length}`);
    lines.push(`{bold}Agents:{/bold} ${state.agents.length}`);
    lines.push("");

    if (state.selectedAgentId) {
      const agent = state.agents.find((a) => a.id === state.selectedAgentId);
      if (agent) {
        lines.push(`{bold}{${agentColor(agent.id)}-fg}${agent.name}{/}`);
        lines.push(`  {gray-fg}ID:{/} ${truncate(agent.id, 20)}`);
        lines.push(`  {gray-fg}Job:{/} ${agent.occupation || "-"}`);
        lines.push(`  {gray-fg}State:{/} ${agent.state || "-"}`);
        lines.push(`  {gray-fg}Activity:{/} ${truncate(agent.current_activity, 28) || "-"}`);
        const loc = state.locations.find((l) => l.id === agent.current_location_id);
        lines.push(`  {gray-fg}At:{/} ${loc ? loc.name : agent.current_location_id}`);
        lines.push(`  {gray-fg}Balance:{/} $${(agent.balance_cents / 100).toFixed(2)}`);
        lines.push(`  {gray-fg}Food:{/} ${agent.food_level}  {gray-fg}Water:{/} ${agent.water_level}`);
        lines.push(`  {gray-fg}Stamina:{/} ${agent.stamina_level}  {gray-fg}Sleep:{/} ${agent.sleep_level}`);
        lines.push("");
      }
    } else {
      lines.push("{gray-fg}Click an agent dot on the map to select.{/}");
      lines.push("");
    }

    if (state.agents.length > 0) {
      lines.push("{bold}Agents:{/bold}");
      for (const agent of state.agents) {
        const loc = state.locations.find((l) => l.id === agent.current_location_id);
        const color = agentColor(agent.id);
        const marker = state.selectedAgentId === agent.id ? "▸" : " ";
        const act = truncate(agent.current_activity, 18) || agent.state || "idle";
        lines.push(`${marker}{${color}-fg}●{/} ${truncate(agent.name, 12)} {gray-fg}@ ${truncate(loc?.name || "?", 10)}{/} — ${act}`);
      }
      lines.push("");
    }

    if (state.events.length > 0) {
      lines.push("{bold}Recent events:{/bold}");
      for (const evt of state.events.slice(0, 6)) {
        const time = formatTime(evt.occurred_at);
        lines.push(`  {gray-fg}${time}{/} ${truncate(evt.description, 30)}`);
      }
    }

    if (state.error) {
      lines.push("");
      lines.push(`{red-fg}{bold}Error:{/} ${state.error}{/}`);
    }

    sidebar.setContent(lines.join("\n"));
  }

  function render() {
    const timeStr = state.worldTime
      ? `${formatTime(state.worldTime.timestamp)} ${state.worldTime.time_of_day || ""}`
      : "connecting...";
    const status = state.error ? `{red-fg}ERROR{/}` : `{green-fg}LIVE{/}`;
    header.setContent(` {bold}letta city{/bold}   ${status}   ${timeStr}   ${state.agents.length} agents   ${state.locations.length} locations`);
    footer.setContent(` {bold}↑↓←→{/bold} scroll  {bold}Tab{/bold} next agent  {bold}Enter{/bold} select  {bold}q{/bold} quit   last update: ${state.lastUpdate ? formatTime(state.lastUpdate) : "-"}`);

    renderMap();
    renderSidebar();
    screen.render();
  }

  async function fetchData() {
    try {
      const headers = { "x-sim-key": simKey };
      const [locRes, agentRes, timeRes, eventsRes] = await Promise.all([
        fetch(`${apiBase}/locations`, { headers }),
        fetch(`${apiBase}/agents`, { headers }),
        fetch(`${apiBase}/world/time`, { headers }),
        fetch(`${apiBase}/events/recent?limit=10`, { headers }),
      ]);

      const locations = (await locRes.json()).data || [];
      const agents = (await agentRes.json()).data || [];
      const worldTime = (await timeRes.json()).data || null;
      const events = (await eventsRes.json()).data || [];

      state = {
        ...state,
        locations,
        agents,
        worldTime,
        events,
        error: null,
        lastUpdate: new Date().toISOString(),
      };
      render();
    } catch (err) {
      state.error = err.message;
      render();
    }
  }

  let pollInterval;
  function startPolling() {
    fetchData();
    pollInterval = setInterval(fetchData, pollMs);
  }

  function stopPolling() {
    clearInterval(pollInterval);
  }

  function selectNextAgent() {
    if (state.agents.length === 0) return;
    const idx = state.selectedAgentId
      ? state.agents.findIndex((a) => a.id === state.selectedAgentId)
      : -1;
    const next = (idx + 1) % state.agents.length;
    state.selectedAgentId = state.agents[next].id;
    render();
  }

  function selectPrevAgent() {
    if (state.agents.length === 0) return;
    const idx = state.selectedAgentId
      ? state.agents.findIndex((a) => a.id === state.selectedAgentId)
      : 0;
    const prev = (idx - 1 + state.agents.length) % state.agents.length;
    state.selectedAgentId = state.agents[prev].id;
    render();
  }

  screen.key(["q", "escape", "C-c"], () => {
    stopPolling();
    screen.destroy();
  });

  screen.key(["tab"], () => {
    selectNextAgent();
  });

  screen.key(["S-tab"], () => {
    selectPrevAgent();
  });

  screen.key(["enter"], () => {
    if (state.selectedAgentId) {
      const agent = state.agents.find((a) => a.id === state.selectedAgentId);
      if (agent) {
        blessed.message({
          parent: screen,
          top: "center",
          left: "center",
          width: 50,
          height: 14,
          tags: true,
          border: { type: "line" },
          style: { border: { fg: "cyan" } },
          content: [
            `{bold}{${agentColor(agent.id)}-fg}${agent.name}{/}`,
            ``,
            `ID: ${truncate(agent.id, 40)}`,
            `Occupation: ${agent.occupation || "-"}`,
            `State: ${agent.state || "-"}`,
            `Activity: ${agent.current_activity || "-"}`,
            `Balance: $${(agent.balance_cents / 100).toFixed(2)}`,
            ``,
            `Food: ${agent.food_level}  Water: ${agent.water_level}`,
            `Stamina: ${agent.stamina_level}  Sleep: ${agent.sleep_level}`,
          ].join("\n"),
        });
        screen.render();
      }
    }
  });

  startPolling();

  return {
    close() {
      stopPolling();
      screen.destroy();
    },
  };
}
