import { resolveRuntimeConfig } from "../config.mjs";
import { invokeDirectAction, waitForInterrupt } from "../runtime/world-api.mjs";

function printJson(value) {
  console.log(JSON.stringify(value, null, 2));
}

function summarizeWaitResult(response) {
  const body = response?.body || {};
  return {
    ok: response.ok,
    status_code: response.status_code,
    status: body.status || "unknown",
    ...(body.result ? { result: body.result } : {}),
    ...(body.interrupt ? { interrupt: body.interrupt } : {}),
    ...(body.error ? { error: body.error } : {}),
  };
}

function normalizeTimeout(value, fallback = 30000) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed < 0) return fallback;
  return Math.floor(parsed);
}

export async function runDirectCitizenCommand({ command, flags }) {
  const resolved = resolveRuntimeConfig({ flags, cwd: flags.cwd || process.cwd() });
  const agentId = resolved.world.auth.city_agent_id.value;

  if (!agentId) {
    throw new Error("missing city agent id; set .lcity/agent_id, profile world.auth.city_agent_id, or --city-agent-id");
  }

  switch (command) {
    case "look-around": {
      const response = await invokeDirectAction(resolved, "look_around", {});
      printJson(response);
      return response.ok ? 0 : 1;
    }

    case "wait": {
      const timeoutMs = normalizeTimeout(flags.timeoutMs ?? flags.timeout, 30000);
      const response = await waitForInterrupt(resolved, agentId, timeoutMs);
      printJson({
        ok: response.ok,
        command: "wait",
        timeout_ms: timeoutMs,
        wait: summarizeWaitResult(response),
      });
      return response.ok ? 0 : 1;
    }

    case "move-to": {
      const locationId = String(flags.locationId || flags.location || "").trim();
      if (!locationId) {
        throw new Error("--location-id is required for move-to");
      }

      const moveResponse = await invokeDirectAction(resolved, "move_to", { location_id: locationId });
      if (!moveResponse.ok) {
        printJson(moveResponse);
        return 1;
      }

      if (flags.wait === false || flags.wait === "false") {
        printJson({
          ok: true,
          command: "move-to",
          move: moveResponse,
        });
        return 0;
      }

      const timeoutMs = normalizeTimeout(flags.timeoutMs ?? flags.timeout, 30000);
      const waitResponse = await waitForInterrupt(resolved, agentId, timeoutMs);
      printJson({
        ok: moveResponse.ok && waitResponse.ok,
        command: "move-to",
        destination_id: locationId,
        move: moveResponse,
        wait: summarizeWaitResult(waitResponse),
      });
      return moveResponse.ok && waitResponse.ok ? 0 : 1;
    }

    default:
      throw new Error(`unknown direct citizen command: ${command}`);
  }
}
