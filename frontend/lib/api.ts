import { Agent, ApiResponse, BootstrapSnapshot, Location, WorldTime } from "@/types/world";

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

export async function fetchLocations(): Promise<Location[]> {
  return fetchJson<Location[]>("/locations");
}

export async function fetchWorldTime(): Promise<WorldTime> {
  return fetchJson<WorldTime>("/world/time");
}

export async function fetchBootstrapSnapshot(): Promise<BootstrapSnapshot> {
  const [agents, locations, worldTime] = await Promise.all([
    fetchAgents(),
    fetchLocations(),
    fetchWorldTime(),
  ]);

  return { agents, locations, worldTime };
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
