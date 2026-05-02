import { resumeSession } from "@letta-ai/letta-code-sdk";
import { buildWorldTools } from "./tool-registry.mjs";
import { fetchToolManifest, invokeCitizenLifecycle } from "./world-api.mjs";

function buildWakeInput(wake, manifest) {
  const locationName = manifest.location_name || wake.agent?.location?.name || wake.agent?.location?.id || "unknown location";
  const triggerKind = wake.trigger?.kind || "unknown";
  const triggerRef = wake.trigger?.ref || "unknown";
  const availableTools = (manifest.tools || []).map((tool) => `- ${tool.name}: ${tool.description}`).join("\n");

  return [
    wake.prompt?.narrative || "You have received a city wake.",
    "",
    "Wake metadata:",
    `- event_id: ${wake.event_id}`,
    `- wake_type: ${wake.type}`,
    `- world_time: ${wake.world_time}`,
    `- location: ${locationName}`,
    `- trigger: ${triggerKind}:${triggerRef}`,
    "",
    "Use only the currently available world tools for world-visible actions.",
    "Freeform text alone does not change shared world state.",
    "",
    "Available world tools:",
    availableTools || "- (none)",
    "",
    `Structured wake payload: ${JSON.stringify(wake.prompt?.structured ?? {})}`,
  ].join("\n");
}

function createToolPolicy(toolNames) {
  const allowed = new Set(toolNames);

  return (toolName) => {
    if (allowed.has(toolName)) {
      return {
        behavior: "allow",
        updatedInput: null,
        updatedPermissions: [],
      };
    }

    return {
      behavior: "deny",
      message: `Tool ${toolName} is not available in the current citizen harness context.`,
    };
  };
}

async function runTurnWithAbort(session, message, signal, options) {
  if (!signal) {
    return session.runTurn(message, options);
  }

  if (signal.aborted) {
    throw new Error("aborted");
  }

  return new Promise((resolve, reject) => {
    let settled = false;

    function cleanup() {
      signal.removeEventListener("abort", onAbort);
    }

    function settle(fn, value) {
      if (settled) return;
      settled = true;
      cleanup();
      fn(value);
    }

    function onAbort() {
      void session.abort().catch(() => {});
      settle(reject, new Error("aborted"));
    }

    signal.addEventListener("abort", onAbort, { once: true });

    session.runTurn(message, options)
      .then((result) => settle(resolve, result))
      .catch((error) => settle(reject, error));
  });
}

export async function processWake(config, wake, tracker, emit, signal) {
  if (signal?.aborted) return;

  if (!wake?.event_id || !wake?.wake_token) {
    emit("wake_error", { eventId: wake?.event_id || "unknown", error: new Error("invalid wake payload") });
    return;
  }

  if (tracker.isClosed(wake.event_id) || tracker.isActive(wake.event_id)) {
    emit("wake_duplicate_ignored", { eventId: wake.event_id });
    return;
  }

  tracker.start(wake.event_id);
  emit("wake_received", {
    event_id: wake.event_id,
    seq: wake.seq,
    type: wake.type,
    narrative: wake.prompt?.narrative || "",
    location: wake.agent?.location?.name || wake.agent?.location?.id || "",
    triggerLabel: `${wake.trigger?.kind || "unknown"}:${wake.trigger?.ref || "unknown"}`,
    expiresAt: wake.wake_token_expires_at || "",
    droppedOverflowCount: wake.meta?.dropped_for_overflow_count || 0,
    agent: wake.agent,
  });

  let wakeClosed = false;

  try {
    const manifest = await fetchToolManifest(config);
    emit("tool_manifest_loaded", {
      eventId: wake.event_id,
      locationId: manifest.location_id,
      locationName: manifest.location_name,
      toolCount: Array.isArray(manifest.tools) ? manifest.tools.length : 0,
    });

    const tools = buildWorldTools({
      config,
      manifest,
      emit,
      wakeEventId: wake.event_id,
    });

    const toolNames = tools.map((tool) => tool.name);
    const session = resumeSession(config.letta.agent_id.value, {
      cwd: process.cwd(),
      permissionMode: "default",
      canUseTool: createToolPolicy(toolNames),
      allowedTools: toolNames,
      tools,
      skillSources: [],
      systemInfoReminder: false,
      memfsStartup: "skip",
      approvalRecoveryTimeoutMs: config.runtime.action_timeout_ms.value,
    });

    try {
      const init = await session.initialize();
      emit("session_initialized", {
        eventId: wake.event_id,
        conversationId: init.conversationId,
        toolCount: toolNames.length,
      });

      const result = await runTurnWithAbort(
        session,
        buildWakeInput(wake, manifest),
        signal,
        {
          maxApprovalRecoveryAttempts: 1,
          recoveryTimeoutMs: config.runtime.action_timeout_ms.value,
        },
      );

      emit("turn_result", {
        eventId: wake.event_id,
        success: result.success,
        durationMs: result.durationMs,
        error: result.errorDetail || result.error || null,
      });

      if (!result.success) {
        throw new Error(result.errorDetail || result.error || "wake turn failed");
      }
    } finally {
      session.close();
    }

    const doneResult = await invokeCitizenLifecycle(config, wake, "wake_done", {});
    if (doneResult.body?.control?.wake_closed) {
      wakeClosed = true;
    }

    tracker.finish(wake.event_id);
    emit("wake_completed", {
      eventId: wake.event_id,
      seq: wake.seq,
      type: wake.type,
      location: wake.agent?.location?.name || wake.agent?.location?.id || "",
      trigger: `${wake.trigger?.kind || "unknown"}:${wake.trigger?.ref || "unknown"}`,
      narrative: wake.prompt?.narrative || "",
      expiresAt: wake.wake_token_expires_at || "",
      wakeClosed,
    });
  } catch (error) {
    tracker.abort(wake.event_id);
    emit("wake_error", { eventId: wake.event_id, error });

    try {
      await invokeCitizenLifecycle(config, wake, "wake_abort", {
        reason: `harness_error:${String(error?.message || error).slice(0, 120)}`,
      });
      emit("wake_abort_sent", { eventId: wake.event_id });
    } catch (abortError) {
      emit("wake_abort_failed", { eventId: wake.event_id, error: abortError });
    }
  }
}
