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

export async function runMockRunCommand({ flags }) {
  const agentId = flags.agentId || process.env.LCITY_AGENT_ID || "";
  const simKey = flags.simKey || process.env.SIM_API_KEY || "";
  const apiBase = normalizeApiBase(flags.apiBase || process.env.LCITY_API_BASE || "http://localhost:8080");
  const autoClose = flags.autoClose !== false && flags.autoClose !== "false";

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

  const config = {
    agentId,
    simKey,
    apiBase,
    wsUrl: wsUrlFromApiBase(apiBase),
    autoClose,
    reconnectInitialMs: 500,
    reconnectMaxMs: 5000,
  };

  const emit = (event, payload = {}) => {
    console.log(JSON.stringify({ ts: new Date().toISOString(), event, ...payload }, null, 2));
  };

  try {
    await runMockHarness(config, emit, controller.signal);
    unbindSignals();
    return 0;
  } catch (error) {
    unbindSignals();
    emit("fatal_error", { error: error.message });
    return 1;
  }
}
