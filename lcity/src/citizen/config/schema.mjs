export const DEFAULTS = {
  world: {
    api_base: "http://localhost:3001",
    tool_manifest_strategy: "server_manifest",
    tool_auth: {
      mode: "sim_key",
    },
  },
  letta: {
    base_url: "https://api.letta.com",
  },
  runtime: {
    max_wake_iterations: 8,
    reconnect_initial_ms: 500,
    reconnect_max_ms: 5000,
    recent_wake_cache_size: 128,
    action_timeout_ms: 15000,
    wake_auto_done: true,
    abort_on_unhandled_tool: true,
    allow_wake_replay_on_reconnect: true,
    max_consecutive_errors: 25,
    pause_on_error: false,
    log_level: "info",
  },
  ui: {
    display_mode: "auto",
    theme: "midnight",
    refresh_ms: 250,
    event_history_limit: 50,
    show_raw_events: false,
  },
};

export const PROFILE_DIR = ".lcity/citizen/profiles";
export const STATE_DIR = ".lcity/citizen/state";
export const ACTIVE_PROFILE_FILE = ".lcity/citizen/state/active_profile";
export const LEGACY_PROFILE_DIR = ".lcity-citizen/profiles";
export const LEGACY_STATE_DIR = ".lcity-citizen/state";
export const LEGACY_ACTIVE_PROFILE_FILE = ".lcity-citizen/state/active_profile";

export function createResolvedValue({ value, source, masked = false, present = true }) {
  return { value, source, masked, present };
}

export function deepClone(value) {
  return JSON.parse(JSON.stringify(value));
}
