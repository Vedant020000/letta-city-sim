import { Agent, AgentIntention, ApiResponse, BootstrapSnapshot, Location, SimEvent, WorldEventEnvelope, WorldTime } from "@/types/world";

function getApiBase() {
  if (process.env.NEXT_PUBLIC_API_URL) {
    return process.env.NEXT_PUBLIC_API_URL.replace(/\/$/, "");
  }

  return process.env.NODE_ENV === "production" ? "/api" : "http://localhost:3001";
}

async function fetchJson<T>(path: string): Promise<T> {
  const response = await fetch(`${getApiBase()}${path}`);
  if (!response.ok) {
    const body = await response.text();
    throw new Error(`Request failed for ${path} (${response.status}): ${body || response.statusText}`);
  }

  return response.json() as Promise<T>;
}

export async function fetchAgents(): Promise<Agent[]> {
  const response = await fetchJson<ApiResponse<Agent[]>>("/agents");
  return response.data;
}

export async function fetchCurrentIntentions(): Promise<AgentIntention[]> {
  const response = await fetchJson<ApiResponse<AgentIntention[]>>("/intentions/current");
  return response.data;
}

export async function fetchLocations(): Promise<Location[]> {
  return fetchJson<Location[]>("/locations");
}

export async function fetchWorldTime(): Promise<WorldTime> {
  return fetchJson<WorldTime>("/world/time");
}

function eventToEnvelope(event: SimEvent): WorldEventEnvelope {
  return {
    id: `event:${event.id}`,
    ts: event.occurred_at,
    type: event.type,
    agent_targets: event.actor_id ? [event.actor_id] : [],
    location_id: event.location_id,
    payload: {
      actor_id: event.actor_id,
      description: event.description,
      metadata: event.metadata,
      source: "history",
    },
  };
}

export async function fetchRecentEvents(limit = 20): Promise<WorldEventEnvelope[]> {
  const events = await fetchJson<SimEvent[]>(`/events?limit=${limit}`);
  return events.map(eventToEnvelope);
}

export async function fetchBootstrapSnapshot(): Promise<BootstrapSnapshot> {
  const [agents, currentIntentions, locations, worldTime, recentEvents] = await Promise.all([
    fetchAgents(),
    fetchCurrentIntentions(),
    fetchLocations(),
    fetchWorldTime(),
    fetchRecentEvents(),
  ]);

  return { agents, currentIntentions, locations, worldTime, recentEvents };
}

export function getWsUrl() {
  if (process.env.NEXT_PUBLIC_WS_URL) {
    return process.env.NEXT_PUBLIC_WS_URL;
  }

  if (typeof window !== "undefined") {
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    return `${protocol}//${window.location.host}/ws/events`;
  }

  return process.env.NODE_ENV === "production" ? "ws://127.0.0.1:3000/ws/events" : "ws://localhost:3001/ws/events";
}
