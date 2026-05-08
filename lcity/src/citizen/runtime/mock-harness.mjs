import WebSocket from "ws";

function sleep(ms, signal) {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) {
      reject(new Error("aborted"));
      return;
    }
    const timeout = setTimeout(() => {
      cleanup();
      resolve();
    }, ms);
    function onAbort() {
      clearTimeout(timeout);
      cleanup();
      reject(new Error("aborted"));
    }
    function cleanup() {
      signal?.removeEventListener("abort", onAbort);
    }
    signal?.addEventListener("abort", onAbort, { once: true });
  });
}

async function closeWake(apiBase, agentId, simKey, eventId) {
  try {
    const res = await fetch(
      `${apiBase}/admin/agents/${encodeURIComponent(agentId)}/citizen-wakes/${encodeURIComponent(eventId)}/close`,
      {
        method: "POST",
        headers: { "x-sim-key": simKey },
      }
    );
    return res.ok;
  } catch {
    return false;
  }
}

async function runSocketSession(config, emit, signal) {
  return new Promise((resolve, reject) => {
    let settled = false;
    const wsUrl = config.wsUrl;

    emit("socket_connecting", { wsUrl });

    const ws = new WebSocket(wsUrl, {
      headers: {
        "x-sim-key": config.simKey,
        "x-agent-id": config.agentId,
      },
    });

    function cleanup() {
      signal?.removeEventListener("abort", onAbort);
      ws.removeAllListeners();
    }

    function settleResolve() {
      if (settled) return;
      settled = true;
      cleanup();
      resolve();
    }

    function settleReject(error) {
      if (settled) return;
      settled = true;
      cleanup();
      reject(error);
    }

    function onAbort() {
      try {
        ws.close();
      } catch {}
      settleResolve();
    }

    signal?.addEventListener("abort", onAbort, { once: true });

    ws.on("open", () => {
      emit("socket_connected", { wsUrl });
    });

    ws.on("message", async (data) => {
      try {
        const wake = JSON.parse(data.toString());
        emit("wake_received", wake);

        if (config.autoClose && wake.event_id) {
          const closed = await closeWake(config.apiBase, config.agentId, config.simKey, wake.event_id);
          emit("wake_closed", { eventId: wake.event_id, closed });
        }
      } catch (error) {
        emit("socket_error", { error: new Error(`invalid websocket message: ${error.message}`) });
      }
    });

    ws.on("error", (error) => {
      emit("socket_error", { error });
    });

    ws.on("close", () => {
      emit("socket_closed", {});
      settleResolve();
    });
  });
}

export async function runMockHarness(config, emit, signal) {
  emit("startup", {
    agentId: config.agentId,
    apiBase: config.apiBase,
    wsUrl: config.wsUrl,
    autoClose: config.autoClose,
  });

  let backoffMs = config.reconnectInitialMs;

  while (!signal?.aborted) {
    const sessionStartedAt = Date.now();
    try {
      await runSocketSession(config, emit, signal);
    } catch (error) {
      emit("socket_error", { error });
    }

    if (signal?.aborted) break;

    const sessionLivedMs = Date.now() - sessionStartedAt;
    if (sessionLivedMs > 5000) {
      backoffMs = config.reconnectInitialMs;
    }

    emit("socket_reconnect_wait", { delayMs: backoffMs });

    try {
      await sleep(backoffMs, signal);
    } catch {
      break;
    }

    backoffMs = Math.min(backoffMs * 2, config.reconnectMaxMs);
  }

  emit("shutdown", {});
}
