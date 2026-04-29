export type NotificationMode = "instant" | "deferred";

export type ApiResponse<T> = {
  data: T;
  notification?: {
    message: string;
    mode: NotificationMode;
    eta_seconds?: number;
  };
};

export type Agent = {
  id: string;
  name: string;
  occupation: string;
  current_location_id: string;
  state: string;
  current_activity: string | null;
  is_npc: boolean;
  is_active: boolean;
  state_updated_at: string;
  balance_cents: number;
  last_income_cents: number | null;
  last_income_reason: string | null;
  last_income_at: string | null;
  last_expense_cents: number | null;
  last_expense_reason: string | null;
  last_expense_at: string | null;
  food_level: number;
  water_level: number;
  stamina_level: number;
  sleep_level: number;
  last_vitals_update: string;
};

export type Location = {
  id: string;
  name: string;
  description: string;
  map_x: number;
  map_y: number;
};

export type WorldTime = {
  timestamp: string;
  time_of_day: string;
  simulation_paused: boolean;
};

export type WorldEventEnvelope = {
  id: string;
  ts: string;
  type?: string;
  event_type?: string;
  agent_targets: string[];
  location_id: string | null;
  payload: Record<string, unknown>;
};

export type BootstrapSnapshot = {
  agents: Agent[];
  locations: Location[];
  worldTime: WorldTime;
};

export type SimConnectionState = "idle" | "loading" | "open" | "closed" | "error";

export type SimState = {
  agents: Agent[];
  locations: Location[];
  worldTime: WorldTime | null;
  recentEvents: WorldEventEnvelope[];
  connectionState: SimConnectionState;
  loading: boolean;
  error: string | null;
  lastSnapshotAt: string | null;
};
