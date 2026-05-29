// DEPRECATION: This wake-driven harness is legacy.
// The preferred path is direct commands (wait, look-around, move-to) which
// bypass the wake loop entirely. This file will be removed once the
// wake-driven path is fully retired.
import { runWakeLoop } from "./wake-client.mjs";

export async function runHarness(config, emit, signal) {
  emit("startup", {
    cityAgentId: config.world.auth.city_agent_id.value,
    lettaAgentId: config.letta.agent_id.value,
    apiBase: config.world.api_base.value,
    wsUrl: config.world.ws_url.value,
    mode: config.mode,
    runtime: `letta_code_sdk_${config.runtime.wake_transport.value}`,
  });

  await runWakeLoop(config, emit, signal);
  emit("shutdown", {});
}
