import { configOverview } from "../config.mjs";

function nowIso() {
  return new Date().toISOString();
}

function limitPush(list, value, limit) {
  list.unshift(value);
  if (list.length > limit) {
    list.length = limit;
  }
}

function safeMessage(input, fallback = "") {
  if (!input) return fallback;
  if (typeof input === "string") return input;
  if (typeof input.message === "string") return input.message;
  return fallback;
}

function summarizeWake(payload = {}) {
  return {
    eventId: payload.event_id || payload.eventId || "",
    seq: payload.seq ?? null,
    type: payload.wake_type || payload.type || "",
    location: payload.location || payload.locationName || payload.agent?.location?.name || payload.agent?.location?.id || "",
    trigger: payload.triggerLabel || payload.trigger || "",
    narrative: payload.narrative || payload.prompt?.narrative || "",
    expiresAt: payload.expiresAt || payload.wake_token_expires_at || "",
    droppedOverflowCount: payload.droppedOverflowCount ?? payload.meta?.dropped_for_overflow_count ?? 0,
  };
}

function eventSummary(event, payload = {}) {
  switch (event) {
    case "startup":
      return `starting harness for ${payload.cityAgentId} -> ${payload.lettaAgentId}`;
    case "tool_manifest_loaded":
      return `loaded ${payload.toolCount} tools for ${payload.locationName || payload.locationId || "current context"}`;
    case "session_initialized":
      return `session ready ${payload.conversationId || ""} (${payload.toolCount || 0} tools)`;
    case "socket_connecting":
      return `connecting websocket: ${payload.wsUrl}`;
    case "socket_connected":
      return `websocket connected`;
    case "socket_closed":
      return `websocket closed`;
    case "socket_error":
      return `websocket error: ${safeMessage(payload.error, "unknown")}`;
    case "socket_reconnect_wait":
      return `reconnect in ${payload.delayMs}ms`;
    case "wake_received":
      return `wake ${payload.event_id} (${payload.type}) received`;
    case "wake_completed":
      return `wake ${payload.eventId} completed`;
    case "wake_duplicate_ignored":
      return `duplicate wake ignored: ${payload.eventId}`;
    case "wake_error":
      return `wake ${payload.eventId} error: ${safeMessage(payload.error, "unknown")}`;
    case "wake_abort_sent":
      return `wake ${payload.eventId} abort sent`;
    case "wake_abort_failed":
      return `wake ${payload.eventId} abort failed: ${safeMessage(payload.error, "unknown")}`;
    case "tool_call_started":
      return `tool ${payload.name} started`;
    case "tool_call_finished":
      return `tool ${payload.name} ${payload.ok ? "ok" : "error"}`;
    case "tool_call_parse_error":
      return `tool args parse error for ${payload.name}: ${safeMessage(payload.error, "unknown")}`;
    case "turn_result":
      return payload.success
        ? `turn completed in ${payload.durationMs || 0}ms`
        : `turn failed: ${payload.error || "unknown"}`;
    case "shutdown":
      return `harness stopped`;
    default:
      return `${event}`;
  }
}

export function createHarnessStore(config) {
  const listeners = new Set();

  const state = {
    startedAt: Date.now(),
    config: configOverview(config),
    connectionState: "starting",
    reconnectDelayMs: 0,
    sessionCount: 0,
    counters: {
      wakesReceived: 0,
      wakesCompleted: 0,
      wakesAborted: 0,
      wakesFailed: 0,
      duplicatesIgnored: 0,
      toolCalls: 0,
    },
    currentWake: null,
    lastWake: null,
    lastAction: null,
    lastError: null,
    recentEvents: [],
    recentActions: [],
  };

  function notify(meta) {
    for (const listener of listeners) {
      listener(state, meta);
    }
  }

  function record(event, payload = {}) {
    const ts = payload.ts || nowIso();

    switch (event) {
      case "socket_connecting":
        state.connectionState = "connecting";
        break;
      case "socket_connected":
        state.connectionState = "connected";
        state.reconnectDelayMs = 0;
        state.sessionCount += 1;
        break;
      case "socket_closed":
        state.connectionState = "disconnected";
        break;
      case "socket_error":
        state.connectionState = "error";
        state.lastError = safeMessage(payload.error, "websocket error");
        break;
      case "socket_reconnect_wait":
        state.connectionState = "reconnect_wait";
        state.reconnectDelayMs = payload.delayMs || 0;
        break;
      case "wake_received":
        state.counters.wakesReceived += 1;
        state.currentWake = summarizeWake(payload);
        break;
      case "wake_duplicate_ignored":
        state.counters.duplicatesIgnored += 1;
        break;
      case "wake_completed":
        state.counters.wakesCompleted += 1;
        state.lastWake = summarizeWake({
          eventId: payload.eventId,
          seq: payload.seq,
          type: payload.type,
          location: payload.location,
          trigger: payload.trigger,
          narrative: payload.narrative,
          expiresAt: payload.expiresAt,
        });
        if (state.currentWake?.eventId === payload.eventId) {
          state.currentWake = null;
        }
        break;
      case "wake_error":
        state.counters.wakesFailed += 1;
        state.lastError = safeMessage(payload.error, "wake error");
        if (state.currentWake?.eventId === payload.eventId) {
          state.lastWake = state.currentWake;
          state.currentWake = null;
        }
        break;
      case "wake_abort_sent":
        state.counters.wakesAborted += 1;
        if (state.currentWake?.eventId === payload.eventId) {
          state.lastWake = state.currentWake;
          state.currentWake = null;
        }
        break;
      case "wake_abort_failed":
        state.lastError = safeMessage(payload.error, "wake abort failed");
        break;
      case "tool_call_started":
        state.counters.toolCalls += 1;
        state.lastAction = `${payload.name} (running)`;
        limitPush(state.recentActions, `${ts.slice(11, 19)} ${payload.name} (running)`, 8);
        break;
      case "tool_call_finished": {
        const message = payload.ok
          ? `${payload.name} -> ok`
          : `${payload.name} -> ${payload.message || "error"}`;
        state.lastAction = message;
        limitPush(state.recentActions, `${ts.slice(11, 19)} ${message}`, 8);
        if (!payload.ok && payload.message) {
          state.lastError = payload.message;
        }
        break;
      }
      case "tool_call_parse_error":
        state.lastError = safeMessage(payload.error, "tool parse error");
        limitPush(state.recentActions, `${ts.slice(11, 19)} ${payload.name} args parse failed`, 8);
        break;
      case "startup":
      case "shutdown":
      default:
        break;
    }

    limitPush(state.recentEvents, `${ts.slice(11, 19)} ${eventSummary(event, payload)}`, 50);
    notify({ event, payload, ts });
  }

  return {
    getState() {
      return state;
    },
    subscribe(listener) {
      listeners.add(listener);
      listener(state, { event: "init", payload: {}, ts: nowIso() });
      return () => listeners.delete(listener);
    },
    record,
  };
}
