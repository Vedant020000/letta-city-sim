import { resolveRuntimeConfig, validateResolvedConfig } from "../config.mjs";
import { createHarnessStore } from "../ui/state.mjs";
import { runHarness } from "../runtime/harness.mjs";

// DEPRECATION: The wake-driven harness (run, interactive, mock-run) is legacy.
// Prefer direct commands: `lcity citizen wait`, `look-around`, `move-to`.
const WAKE_HARNESS_DEPRECATION =
  "[deprecation] The wake-driven harness is legacy. Prefer direct commands: " +
  "lcity citizen wait, look-around, move-to. The harness will be removed in a future release.";

function bindShutdownSignals(controller) {
  const onSignal = () => controller.abort();
  process.once("SIGINT", onSignal);
  process.once("SIGTERM", onSignal);

  return () => {
    process.removeListener("SIGINT", onSignal);
    process.removeListener("SIGTERM", onSignal);
  };
}

function toPlainEvent(event, payload = {}) {
  const plain = { ts: new Date().toISOString(), event };
  for (const [key, value] of Object.entries(payload)) {
    plain[key] = value instanceof Error ? value.message : value;
  }
  return plain;
}

export async function runRunCommand({ flags }) {
  let ui = null;
  let unbindSignals = () => {};

  // Emit deprecation notice so callers know to migrate
  if (flags.plain || !process.stdout.isTTY) {
    console.log(JSON.stringify({ ts: new Date().toISOString(), event: "deprecation_warning", message: WAKE_HARNESS_DEPRECATION }));
  } else {
    console.error(WAKE_HARNESS_DEPRECATION);
  }

  try {
    const resolved = resolveRuntimeConfig({ flags, cwd: flags.cwd || process.cwd() });
    const validation = validateResolvedConfig(resolved);
    if (!validation.ok) {
      console.log(JSON.stringify({ ok: false, validation }, null, 2));
      return 1;
    }

    const controller = new AbortController();
    const store = createHarnessStore(resolved);
    unbindSignals = bindShutdownSignals(controller);

    const emit = (event, payload = {}) => {
      store.record(event, payload);
      if (resolved.ui.display_mode.value === "plain") {
        console.log(JSON.stringify(toPlainEvent(event, payload)));
      }
    };

    if (["tui", "interactive"].includes(resolved.ui.display_mode.value)) {
      const { startTui } = await import("../ui/tui.mjs");
      ui = await startTui(store, {
        onExit: () => controller.abort(),
      });
    }

    await runHarness(resolved, emit, controller.signal);
    ui?.close();
    unbindSignals();
    return 0;
  } catch (error) {
    ui?.close?.();
    unbindSignals();
    console.log(JSON.stringify({ ok: false, error: error.message }, null, 2));
    return 1;
  }
}
