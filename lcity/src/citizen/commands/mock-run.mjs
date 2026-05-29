// DEPRECATION: mock-run is a debug harness for the legacy wake-driven system.
// Prefer direct commands: `lcity citizen wait`, `look-around`, `move-to`.
import { createHarnessStore } from "../ui/state.mjs";
import { runMockHarness } from "../runtime/mock-harness.mjs";

function bindShutdownSignals(controller) {
  const onSignal = () => controller.abort();
  process.once("SIGINT", onSignal);
  process.once("SIGTERM", onSignal);

  return () => {
    process.removeListener("SIGINT", onSignal);
    process.removeListener("SIGTERM", onSignal);
  };
}

function normalizeApiBase(apiBase) {
  return String(apiBase || "http://localhost:8080").trim().replace(/\/$/, "");
}

function wsUrlFromApiBase(apiBase) {
  const trimmed = normalizeApiBase(apiBase);
  if (trimmed.startsWith("https://")) return `wss://${trimmed.slice("https://".length)}/ws/citizen`;
  if (trimmed.startsWith("http://")) return `ws://${trimmed.slice("http://".length)}/ws/citizen`;
  return `${trimmed}/ws/citizen`;
}

function buildMockConfigOverview(config) {
  return [
    { label: "Mode", value: "mock-run", source: "flag" },
    { label: "World API", value: config.apiBase, source: "flag:--api-base" },
    { label: "Citizen WS", value: config.wsUrl, source: "derived" },
    { label: "Agent ID", value: config.agentId, source: "flag:--agent-id" },
    { label: "Sim key", value: "present", source: "flag:--sim-key" },
    { label: "Auto close", value: String(config.autoClose), source: "flag:--auto-close" },
    { label: "Display", value: config.displayMode, source: "flag" },
  ];
}

export async function runMockRunCommand({ flags }) {
  const agentId = flags.agentId || process.env.LCITY_AGENT_ID || "";
  const simKey = flags.simKey || process.env.SIM_API_KEY || "";
  const apiBase = normalizeApiBase(flags.apiBase || process.env.LCITY_API_BASE || "http://localhost:8080");
  const autoClose = flags.autoClose !== false && flags.autoClose !== "false";
  const displayMode = flags.tui ? "tui" : "plain";

  if (!agentId) {
    console.log(JSON.stringify({ ok: false, error: "--agent-id or LCITY_AGENT_ID is required" }, null, 2));
    return 1;
  }
  if (!simKey) {
    console.log(JSON.stringify({ ok: false, error: "--sim-key or SIM_API_KEY is required" }, null, 2));
    return 1;
  }

  const controller = new AbortController();
  const unbindSignals = bindShutdownSignals(controller);
  let ui = null;

  const config = {
    agentId,
    simKey,
    apiBase,
    wsUrl: wsUrlFromApiBase(apiBase),
    autoClose,
    reconnectInitialMs: 500,
    reconnectMaxMs: 5000,
    displayMode,
  };

  const store = createHarnessStore({
    mode: "mock-run",
    mode_entry: { source: "flag" },
    profile: { name: "none", source: "none", present: false },
    world: {
      api_base: { value: config.apiBase, source: "flag:--api-base" },
      ws_url: { value: config.wsUrl, source: "derived" },
      tool_manifest_strategy: { value: "none", source: "mock" },
      auth: {
        city_agent_id: { value: config.agentId, source: "flag:--agent-id" },
        bearer_token: { value: "", source: "none", present: false },
      },
      tool_auth: {
        sim_api_key: { value: config.simKey, source: "flag:--sim-key", present: true },
      },
    },
    letta: {
      base_url: { value: "", source: "none" },
      api_key: { value: "", source: "none", present: false },
      agent_id: { value: "", source: "none" },
    },
    runtime: {
      max_wake_iterations: { value: 0, source: "mock" },
    },
    ui: {
      display_mode: { value: displayMode, source: "flag" },
    },
  });

  const emit = (event, payload = {}) => {
    store.record(event, payload);
    if (displayMode === "plain") {
      console.log(JSON.stringify({ ts: new Date().toISOString(), event, ...payload }, null, 2));
    }
  };

  if (displayMode === "tui") {
    const { startTui } = await import("../ui/tui.mjs");
    ui = await startTui(store, {
      onExit: () => controller.abort(),
    });
  }

  try {
    await runMockHarness(config, emit, controller.signal);
    ui?.close();
    unbindSignals();
    return 0;
  } catch (error) {
    ui?.close?.();
    unbindSignals();
    emit("fatal_error", { error: error.message });
    return 1;
  }
}
