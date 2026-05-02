function formatDuration(ms) {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) return `${hours}h ${minutes}m ${seconds}s`;
  if (minutes > 0) return `${minutes}m ${seconds}s`;
  return `${seconds}s`;
}

function statusColor(connectionState) {
  switch (connectionState) {
    case "connected": return "green";
    case "connecting": return "yellow";
    case "reconnect_wait": return "yellow";
    case "error": return "red";
    default: return "white";
  }
}

function formatKeyValue(lines, key, value) {
  lines.push(`{bold}${key}:{/bold} ${value}`);
}

function joinOrPlaceholder(items, placeholder = "(none)") {
  return items && items.length > 0 ? items.join("\n") : placeholder;
}

function renderStatus(state) {
  const lines = [];
  formatKeyValue(lines, "Connection", `{${statusColor(state.connectionState)}-fg}${state.connectionState}{/}`);
  formatKeyValue(lines, "Uptime", formatDuration(Date.now() - state.startedAt));
  formatKeyValue(lines, "Sessions", String(state.sessionCount));
  formatKeyValue(lines, "Reconnect", state.reconnectDelayMs ? `${state.reconnectDelayMs}ms` : "-" );
  lines.push("");
  formatKeyValue(lines, "Wakes recv", String(state.counters.wakesReceived));
  formatKeyValue(lines, "Completed", String(state.counters.wakesCompleted));
  formatKeyValue(lines, "Aborted", String(state.counters.wakesAborted));
  formatKeyValue(lines, "Errors", String(state.counters.wakesFailed));
  formatKeyValue(lines, "Duplicates", String(state.counters.duplicatesIgnored));
  formatKeyValue(lines, "Tool calls", String(state.counters.toolCalls));
  lines.push("");
  formatKeyValue(lines, "Last action", state.lastAction || "-");
  formatKeyValue(lines, "Last error", state.lastError || "-");
  return lines.join("\n");
}

function renderConfig(state) {
  return state.config
    .map((entry) => `{bold}${entry.label}:{/bold} ${entry.value}\n  {gray-fg}${entry.source}{/}`)
    .join("\n\n");
}

function renderWake(wake, title) {
  if (!wake) return `${title}: (none)`;

  const lines = [];
  formatKeyValue(lines, title, `${wake.eventId || "-"}`);
  formatKeyValue(lines, "Seq", wake.seq == null ? "-" : String(wake.seq));
  formatKeyValue(lines, "Type", wake.type || "-");
  formatKeyValue(lines, "Location", wake.location || "-");
  formatKeyValue(lines, "Trigger", wake.trigger || "-");
  formatKeyValue(lines, "Expires", wake.expiresAt || "-");
  formatKeyValue(lines, "Dropped", String(wake.droppedOverflowCount || 0));
  lines.push("");
  lines.push("{bold}Narrative:{/bold}");
  lines.push(wake.narrative || "-");
  return lines.join("\n");
}

export async function startTui(store, { onExit } = {}) {
  const blessedModule = await import("blessed");
  const blessed = blessedModule.default ?? blessedModule;

  const screen = blessed.screen({
    smartCSR: true,
    fullUnicode: true,
    title: "lcity citizen",
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

  const statusBox = blessed.box({
    parent: screen,
    label: " Status ",
    top: 3,
    left: 0,
    width: "38%",
    height: 15,
    tags: true,
    border: "line",
    padding: { left: 1, right: 1 },
    scrollable: true,
    alwaysScroll: true,
  });

  const configBox = blessed.box({
    parent: screen,
    label: " Config sources ",
    top: 18,
    left: 0,
    width: "38%",
    height: "100%-22",
    tags: true,
    border: "line",
    padding: { left: 1, right: 1 },
    scrollable: true,
    alwaysScroll: true,
  });

  const currentWakeBox = blessed.box({
    parent: screen,
    label: " Current wake ",
    top: 3,
    left: "38%",
    width: "62%",
    height: 15,
    tags: true,
    border: "line",
    padding: { left: 1, right: 1 },
    scrollable: true,
    alwaysScroll: true,
  });

  const actionsBox = blessed.box({
    parent: screen,
    label: " Recent actions / last wake ",
    top: 18,
    left: "38%",
    width: "62%",
    height: 10,
    tags: true,
    border: "line",
    padding: { left: 1, right: 1 },
    scrollable: true,
    alwaysScroll: true,
  });

  const logBox = blessed.box({
    parent: screen,
    label: " Recent events ",
    top: 28,
    left: "38%",
    width: "62%",
    height: "100%-29",
    tags: true,
    border: "line",
    padding: { left: 1, right: 1 },
    scrollable: true,
    alwaysScroll: true,
  });

  const footer = blessed.box({
    parent: screen,
    bottom: 0,
    left: 0,
    width: "100%",
    height: 1,
    tags: true,
    style: { fg: "black", bg: "white" },
    content: " q / esc / ctrl+c quit ",
  });

  function render(state) {
    const connection = `{${statusColor(state.connectionState)}-fg}${state.connectionState}{/}`;
    header.setContent(` {bold}lcity citizen{/bold}   ${connection}   uptime ${formatDuration(Date.now() - state.startedAt)}`);
    statusBox.setContent(renderStatus(state));
    configBox.setContent(renderConfig(state));
    currentWakeBox.setContent(renderWake(state.currentWake, "Event"));

    const actionLines = [];
    actionLines.push("{bold}Recent actions:{/bold}");
    actionLines.push(joinOrPlaceholder(state.recentActions));
    actionLines.push("");
    actionLines.push(renderWake(state.lastWake, "Last wake"));
    actionsBox.setContent(actionLines.join("\n"));

    logBox.setContent(joinOrPlaceholder(state.recentEvents));
    screen.render();
  }

  const unsubscribe = store.subscribe((state) => {
    render(state);
  });

  function close() {
    unsubscribe();
    screen.destroy();
  }

  screen.key(["q", "escape", "C-c"], () => {
    onExit?.();
  });

  screen.on("resize", () => {
    render(store.getState());
  });

  render(store.getState());

  return { close };
}
