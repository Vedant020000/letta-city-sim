import { BootstrapSnapshot, SimConnectionState, SimState, WorldEventEnvelope } from "@/types/world";

type SimAction =
  | { type: "bootstrap_started" }
  | { type: "bootstrap_succeeded"; payload: BootstrapSnapshot }
  | { type: "bootstrap_failed"; error: string }
  | { type: "snapshot_refreshed"; payload: BootstrapSnapshot }
  | { type: "connection_state_changed"; payload: SimConnectionState }
  | { type: "event_received"; payload: WorldEventEnvelope }
  | { type: "error"; error: string };

export const initialSimState: SimState = {
  agents: [],
  locations: [],
  worldTime: null,
  recentEvents: [],
  connectionState: "idle",
  loading: true,
  error: null,
  lastSnapshotAt: null,
};

function resolveEventType(event: WorldEventEnvelope) {
  return event.type || event.event_type || "unknown";
}

function applyEvent(state: SimState, event: WorldEventEnvelope): SimState {
  const eventType = resolveEventType(event);

  const agents = state.agents.map((agent) => {
    if (eventType === "location.enter") {
      const payload = event.payload as { agent_id?: string; to_location_id?: string };
      if (payload.agent_id === agent.id && payload.to_location_id) {
        return {
          ...agent,
          current_location_id: payload.to_location_id,
          state: "walking",
        };
      }
    }

    return agent;
  });

  return {
    ...state,
    agents,
    recentEvents: [event, ...state.recentEvents].slice(0, 40),
  };
}

export function simReducer(state: SimState, action: SimAction): SimState {
  switch (action.type) {
    case "bootstrap_started":
      return {
        ...state,
        loading: true,
        error: null,
        connectionState: state.connectionState === "idle" ? "loading" : state.connectionState,
      };
    case "bootstrap_succeeded":
      return {
        ...state,
        agents: action.payload.agents,
        locations: action.payload.locations,
        worldTime: action.payload.worldTime,
        recentEvents: action.payload.recentEvents,
        loading: false,
        error: null,
        lastSnapshotAt: new Date().toISOString(),
      };
    case "snapshot_refreshed":
      return {
        ...state,
        agents: action.payload.agents,
        locations: action.payload.locations,
        worldTime: action.payload.worldTime,
        lastSnapshotAt: new Date().toISOString(),
      };
    case "bootstrap_failed":
      return {
        ...state,
        loading: false,
        error: action.error,
        connectionState: "error",
      };
    case "connection_state_changed":
      return {
        ...state,
        connectionState: action.payload,
      };
    case "event_received":
      return applyEvent(state, action.payload);
    case "error":
      return {
        ...state,
        error: action.error,
      };
    default:
      return state;
  }
}
