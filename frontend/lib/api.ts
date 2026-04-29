import { Agent, ApiResponse, BootstrapSnapshot, Location, WorldTime } from "@/types/world";

function getApiBase() {
  return process.env.NEXT_PUBLIC_API_URL || "http://localhost:3001";
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
  return process.env.NEXT_PUBLIC_WS_URL || "ws://localhost:3001/ws/events";
}
