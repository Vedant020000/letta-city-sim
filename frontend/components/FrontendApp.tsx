"use client";

import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { PhaserMap } from "@/components/PhaserMap";
import { fetchBootstrapSnapshot, getWsUrl } from "@/lib/api";
import { createMockSnapshotLoop, MockSnapshotLoop } from "@/lib/mock-snapshot";
import { initialSimState, simReducer } from "@/lib/sim-store";
import { connectWorldEvents } from "@/lib/ws-client";

/**
 * Returns true if the error looks like a backend/database failure that
 * should trigger mock mode (500 status, "database error", etc.).
 */
function isBackendFailure(error: unknown): boolean {
  if (!(error instanceof Error)) return true;
  const msg = error.message.toLowerCase();
  return (
    msg.includes("500") ||
    msg.includes("database error") ||
    msg.includes("failed to fetch") ||
    msg.includes("networkerror") ||
    msg.includes("network request failed") ||
    msg.includes("econnrefused") ||
    msg.includes("err_connection_refused")
  );
}

export function FrontendApp() {
  const [state, dispatch] = useReducer(simReducer, initialSimState);
  const refreshTimerRef = useRef<number | null>(null);
  const mockLoopRef = useRef<MockSnapshotLoop | null>(null);
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(null);

  const handleAgentClick = useCallback((agentId: string) => {
    setSelectedAgentId((prev) => (prev === agentId ? null : agentId));
  }, []);

  // ── Mock mode activation ──

  const activateMockMode = useCallback(() => {
    if (mockLoopRef.current) return; // already active
    const loop = createMockSnapshotLoop();
    mockLoopRef.current = loop;
    dispatch({ type: "mock_mode_activated", payload: loop.getSnapshot() });
    // Auto-tick every 8 seconds to keep the demo alive
    loop.start(8000);
    // Also set up an interval to dispatch ticked snapshots
    const tickInterval = setInterval(() => {
      if (mockLoopRef.current) {
        dispatch({ type: "mock_snapshot_ticked", payload: mockLoopRef.current.getSnapshot() });
      }
    }, 8000);
    // Store cleanup ref
    mockTickIntervalRef.current = tickInterval;
  }, []);

  const mockTickIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // ── Snapshot loading with fallback ──

  const loadSnapshot = useCallback(async (mode: "bootstrap" | "refresh") => {
    try {
      if (mode === "bootstrap") {
        dispatch({ type: "bootstrap_started" });
      }

      const snapshot = await fetchBootstrapSnapshot();
      dispatch({ type: mode === "bootstrap" ? "bootstrap_succeeded" : "snapshot_refreshed", payload: snapshot });
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown snapshot error";

      // If this looks like a backend/database failure, activate mock mode
      if (isBackendFailure(error)) {
        activateMockMode();
      } else {
        dispatch({ type: mode === "bootstrap" ? "bootstrap_failed" : "error", error: message });
      }
    }
  }, [activateMockMode]);

  const scheduleRefresh = useCallback(() => {
    if (refreshTimerRef.current !== null) {
      window.clearTimeout(refreshTimerRef.current);
    }

    refreshTimerRef.current = window.setTimeout(() => {
      loadSnapshot("refresh");
    }, 500);
  }, [loadSnapshot]);

  useEffect(() => {
    loadSnapshot("bootstrap");
  }, [loadSnapshot]);

  useEffect(() => {
    // Skip WS connection in mock mode
    if (state.mockMode) return;

    const disconnect = connectWorldEvents({
      url: getWsUrl(),
      onOpen: () => dispatch({ type: "connection_state_changed", payload: "open" }),
      onClose: () => dispatch({ type: "connection_state_changed", payload: "closed" }),
      onError: (error) => {
        dispatch({ type: "connection_state_changed", payload: "error" });
        dispatch({ type: "error", error });
      },
      onEvent: (event) => {
        dispatch({ type: "event_received", payload: event });
        scheduleRefresh();
      },
    });

    return () => {
      disconnect();
      if (refreshTimerRef.current !== null) {
        window.clearTimeout(refreshTimerRef.current);
      }
    };
  }, [scheduleRefresh, state.mockMode]);

  // Cleanup mock loop on unmount
  useEffect(() => {
    return () => {
      if (mockLoopRef.current) {
        mockLoopRef.current.stop();
        mockLoopRef.current = null;
      }
      if (mockTickIntervalRef.current) {
        clearInterval(mockTickIntervalRef.current);
        mockTickIntervalRef.current = null;
      }
    };
  }, []);

  return (
    <PhaserMap
      agents={state.agents}
      locations={state.locations}
      onAgentClick={handleAgentClick}
      selectedAgentId={selectedAgentId}
    />
  );
}
