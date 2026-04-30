"use client";

import { useCallback, useEffect, useReducer, useRef } from "react";
import { EventFeed } from "@/components/EventFeed";
import { PhaserMap } from "@/components/PhaserMap";
import { TownPulsePanel } from "@/components/TownPulsePanel";
import { fetchBootstrapSnapshot, getWsUrl } from "@/lib/api";
import { initialSimState, simReducer } from "@/lib/sim-store";
import { connectWorldEvents } from "@/lib/ws-client";

function formatWorldTime(value: string | null) {
  if (!value) return "-";
  return new Date(value).toLocaleString();
}

export function FrontendApp() {
  const [state, dispatch] = useReducer(simReducer, initialSimState);
  const refreshTimerRef = useRef<number | null>(null);
  const intentionsByAgent = new Map(state.currentIntentions.map((intention) => [intention.agent_id, intention]));

  const loadSnapshot = useCallback(async (mode: "bootstrap" | "refresh") => {
    try {
      if (mode === "bootstrap") {
        dispatch({ type: "bootstrap_started" });
      }

      const snapshot = await fetchBootstrapSnapshot();
      dispatch({ type: mode === "bootstrap" ? "bootstrap_succeeded" : "snapshot_refreshed", payload: snapshot });
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown snapshot error";
      dispatch({ type: mode === "bootstrap" ? "bootstrap_failed" : "error", error: message });
    }
  }, []);

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
  }, [scheduleRefresh]);

  return (
    <main className="page">
      <div className="shell">
        <section className="hero">
          <div className="eyebrow">maintainer-owned frontend engine MVP</div>
          <h1>Simulation frontend foundation</h1>
          <p>
            This is the minimum viable frontend engine for letta-city-sim: bootstrap from the World API,
            subscribe to <code>/ws/events</code>, render a placeholder Phaser town surface, and expose a raw event feed for debugging.
          </p>
          <div className="status-grid">
            <div className="stat-card">
              <span className="stat-label">World time</span>
              <span className="stat-value small">{state.worldTime ? `${state.worldTime.time_of_day} · ${formatWorldTime(state.worldTime.timestamp)}` : "loading"}</span>
            </div>
            <div className="stat-card">
              <span className="stat-label">Locations</span>
              <span className="stat-value">{state.locations.length}</span>
            </div>
            <div className="stat-card">
              <span className="stat-label">Agents</span>
              <span className="stat-value">{state.agents.length}</span>
            </div>
            <div className="stat-card">
              <span className="stat-label">Event stream</span>
              <span className={`connection-pill ${state.connectionState}`}>{state.connectionState}</span>
            </div>
          </div>
        </section>

        <TownPulsePanel pulse={state.townPulse} />

        {state.loading ? <div className="loading">Bootstrapping world snapshot...</div> : null}
        {state.error ? <div className="error-box">{state.error}</div> : null}

        <div className="layout-grid">
          <div className="column">
            <section className="map-shell">
              <div className="map-shell-header">
                <div className="map-shell-title">
                  <strong>Town view</strong>
                  <span>Phaser placeholder renderer using location map_x/map_y anchors and agent markers.</span>
                </div>
                <span className={`connection-pill ${state.connectionState}`}>{state.connectionState}</span>
              </div>
              <div className="map-frame">
                <PhaserMap agents={state.agents} locations={state.locations} />
              </div>
            </section>

            <section className="panel">
              <h2>Agents in snapshot</h2>
              <div className="agent-list">
                {state.agents.map((agent) => (
                  <div className="agent-row" key={agent.id}>
                    <div>
                      <strong>{agent.name}</strong>
                      <small>
                        {agent.occupation} · {agent.current_location_id}
                      </small>
                      {intentionsByAgent.get(agent.id) ? (
                        <small className="agent-intention">
                          Intends: {intentionsByAgent.get(agent.id)?.summary}
                          <span>{intentionsByAgent.get(agent.id)?.reason}</span>
                        </small>
                      ) : null}
                    </div>
                    <small>{agent.current_activity || agent.state}</small>
                  </div>
                ))}
              </div>
            </section>
          </div>

          <div className="column">
            <section className="panel">
              <h2>Raw websocket event feed</h2>
              <p className="muted">
                This is intentionally unpolished. It exists to prove the browser can consume the World API event stream end-to-end.
              </p>
              <EventFeed events={state.recentEvents} />
            </section>
          </div>
        </div>
      </div>
    </main>
  );
}
