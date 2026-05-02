import WebSocket from "ws";
import { processWake } from "./session-runner.mjs";

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

function createRecentWakeTracker(limit = 128) {
  const closed = new Set();
  const order = [];
  const active = new Set();

  return {
    isClosed(eventId) {
      return closed.has(eventId);
    },
    isActive(eventId) {
      return active.has(eventId);
    },
    start(eventId) {
      active.add(eventId);
    },
    finish(eventId) {
      active.delete(eventId);
      if (closed.has(eventId)) return;
      closed.add(eventId);
      order.push(eventId);
      while (order.length > limit) {
        const oldest = order.shift();
        closed.delete(oldest);
      }
    },
    abort(eventId) {
      active.delete(eventId);
    },
  };
}

async function runSocketSession(config, tracker, emit, signal) {
  return new Promise((resolve, reject) => {
    let settled = false;
    let processing = false;
    const queue = [];

    emit("socket_connecting", { wsUrl: config.world.ws_url.value });

    const ws = new WebSocket(config.world.ws_url.value, {
      headers: {
        Authorization: `Bearer ${config.world.auth.bearer_token.value}`,
        "x-agent-id": config.world.auth.city_agent_id.value,
      },
    });

    async function drainQueue() {
      if (processing || signal?.aborted) return;
      processing = true;
      try {
        while (queue.length > 0 && !signal?.aborted) {
          const wake = queue.shift();
          await processWake(config, wake, tracker, emit, signal);
        }
      } catch (error) {
        settleReject(error);
      } finally {
        processing = false;
      }
    }

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
      } catch {
        // ignore
      }
      settleResolve();
    }

    signal?.addEventListener("abort", onAbort, { once: true });

    ws.on("open", () => {
      emit("socket_connected", { wsUrl: config.world.ws_url.value });
    });

    ws.on("message", (data) => {
      try {
        const wake = JSON.parse(data.toString());
        queue.push(wake);
        void drainQueue();
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

export async function runWakeLoop(config, emit, signal) {
  const tracker = createRecentWakeTracker(config.runtime.recent_wake_cache_size.value);

  let backoffMs = config.runtime.reconnect_initial_ms.value;
  while (!signal?.aborted) {
    const sessionStartedAt = Date.now();
    try {
      await runSocketSession(config, tracker, emit, signal);
    } catch (error) {
      emit("socket_error", { error });
    }

    if (signal?.aborted) break;

    const sessionLivedMs = Date.now() - sessionStartedAt;
    if (sessionLivedMs > 5000) {
      backoffMs = config.runtime.reconnect_initial_ms.value;
    }

    emit("socket_reconnect_wait", { delayMs: backoffMs });

    try {
      await sleep(backoffMs, signal);
    } catch {
      break;
    }

    backoffMs = Math.min(backoffMs * 2, config.runtime.reconnect_max_ms.value);
  }
}
