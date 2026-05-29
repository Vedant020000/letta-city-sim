"use client";

import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { AgentInspector } from "@/components/AgentInspector";
import { EventFeed } from "@/components/EventFeed";
import { PhaserMap } from "@/components/PhaserMap";
import { TownPulsePanel } from "@/components/TownPulsePanel";
import { fetchBootstrapSnapshot, getWsUrl } from "@/lib/api";
import { initialSimState, simReducer } from "@/lib/sim-store";
import { connectWorldEvents } from "@/lib/ws-client";
import { Agent, Location } from "@/types/world";

function formatWorldTime(value: string | null) {
  if (!value) return "—";
  return new Date(value).toLocaleString();
}

export function FrontendApp() {
  const [state, dispatch] = useReducer(simReducer, initialSimState);
  const refreshTimerRef = useRef<number | null>(null);
  const [selectedAgentId, setSelectedAgentId] = useState<string | null>(null);
  const [showEventFeed, setShowEventFeed] = useState(false);

  const selectedAgent = selectedAgentId
    ? state.agents.find((a) => a.id === selectedAgentId) ?? null
    : null;

  const handleAgentClick = useCallback((agentId: string) => {
    setSelectedAgentId((prev) => (prev === agentId ? null : agentId));
  }, []);

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
        {/* Compact header */}
        <header className="top-bar">
          <div className="top-bar-left">
            <span className="logo">⬡</span>
            <div>
              <h1>letta-city-sim</h1>
              <span className="top-bar-subtitle">autonomous AI agents in a living town</span>
            </div>
          </div>
          <div className="top-bar-stats">
            <div className="stat-chip">
              <span className="stat-chip-label">Time</span>
              <span className="stat-chip-value">{state.worldTime ? state.worldTime.time_of_day : "—"}</span>
            </div>
            <div className="stat-chip">
              <span className="stat-chip-label">Agents</span>
              <span className="stat-chip-value">{state.agents.length}</span>
            </div>
            <div className="stat-chip">
              <span className="stat-chip-label">Locations</span>
              <span className="stat-chip-value">{state.locations.length}</span>
            </div>
            <span className={`connection-pill ${state.connectionState}`}>{state.connectionState}</span>
          </div>
        </header>

        {state.loading ? <div className="loading">Bootstrapping world snapshot...</div> : null}
        {state.error ? <div className="error-box">{state.error}</div> : null}

        {/* Town pulse */}
        <TownPulsePanel pulse={state.townPulse} />

        {/* Main layout: map + sidebar */}
        <div className="layout-grid">
          <div className="column">
            <section className="map-shell">
              <div className="map-shell-header">
                <div className="map-shell-title">
                  <strong>Town view</strong>
                  <span>Click an agent to inspect</span>
                </div>
              </div>
              <div className="map-frame">
                <PhaserMap
                  agents={state.agents}
                  locations={state.locations}
                  onAgentClick={handleAgentClick}
                  selectedAgentId={selectedAgentId}
                />
              </div>
            </section>

            {/* Agent roster — compact */}
            <section className="panel">
              <div className="panel-header">
                <h2>Agents</h2>
                <button
                  className={`toggle-btn ${showEventFeed ? "active" : ""}`}
                  onClick={() => setShowEventFeed(!showEventFeed)}
                >
                  {showEventFeed ? "Hide events" : "Show events"}
                </button>
              </div>
              <div className="agent-list">
                {state.agents.map((agent) => (
                  <div
                    className={`agent-row ${selectedAgentId === agent.id ? "selected" : ""}`}
                    key={agent.id}
                    onClick={() => handleAgentClick(agent.id)}
                  >
                    <div className="agent-row-left">
                      <span className="agent-dot" style={{ background: `#${colorForAgentHex(agent.id)}` }} />
                      <div>
                        <strong>{agent.name}</strong>
                        <small>{agent.occupation}</small>
                      </div>
                    </div>
                    <div className="agent-row-right">
                      <span className={`state-badge small ${agent.state}`}>{agent.state}</span>
                      {(agent.state === "traveling" || agent.state === "walking") && agent.travel_destination_id ? (
                        <small className="travel-destination-inline">
                          → {locationNameForId(agent.travel_destination_id, state.locations)}
                          {agent.travel_arrives_at && <span className="travel-eta-inline"> {formatETA(agent.travel_arrives_at)}</span>}
                          {!agent.travel_arrives_at && agent.travel_total_secs != null && <span className="travel-eta-inline"> ~{agent.travel_total_secs}s</span>}
                        </small>
                      ) : (
                        agent.current_activity && <small>{agent.current_activity}</small>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            </section>
          </div>

          <div className="column sidebar">
            {selectedAgent ? (
              <AgentInspector agent={selectedAgent} locations={state.locations} onClose={() => setSelectedAgentId(null)} />
            ) : (
              <section className="panel inspector-empty">
                <p className="muted">Click an agent on the map or in the roster to inspect them.</p>
              </section>
            )}

            {showEventFeed && (
              <section className="panel">
                <h2>Event stream</h2>
                <EventFeed events={state.recentEvents} />
              </section>
            )}
          </div>
        </div>
      </div>
    </main>
  );
}

function colorForAgentHex(agentId: string) {
  const palette = ["3b82f6", "ef4444", "22c55e", "a855f7", "f97316", "06b6d4", "ec4899", "eab308"];
  let hash = 0;
  for (const char of agentId) {
    hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  }
  return palette[hash % palette.length];
}

function locationNameForId(locationId: string | null, locations: Location[]): string {
  if (!locationId) return "—";
  const loc = locations.find((l) => l.id === locationId);
  return loc ? loc.name : locationId;
}

function formatETA(arrivesAt: string | null): string {
  if (!arrivesAt) return "";
  const arrival = new Date(arrivesAt).getTime();
  const now = Date.now();
  const remaining = Math.max(0, Math.round((arrival - now) / 1000));
  if (remaining < 60) return `~${remaining}s`;
  const mins = Math.floor(remaining / 60);
  const secs = remaining % 60;
  return `~${mins}m ${secs}s`;
}
