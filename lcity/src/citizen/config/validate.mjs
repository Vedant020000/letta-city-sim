function isNonEmptyString(value) {
  return typeof value === "string" && value.trim().length > 0;
}

function isBoolean(value) {
  return typeof value === "boolean";
}

function isPositiveInteger(value) {
  return Number.isInteger(value) && value > 0;
}

function isUrl(value) {
  try {
    const parsed = new URL(value);
    return parsed.protocol === "http:" || parsed.protocol === "https:" || parsed.protocol === "ws:" || parsed.protocol === "wss:";
  } catch {
    return false;
  }
}

export function validateResolvedConfig(resolved) {
  const errors = [];
  const warnings = [];

  if (!isNonEmptyString(resolved.mode)) {
    errors.push("mode is required");
  }

  if (!isNonEmptyString(resolved.world.api_base.value) || !isUrl(resolved.world.api_base.value)) {
    errors.push("world.api_base must be a valid URL");
  }

  if (!isNonEmptyString(resolved.world.ws_url.value) || !isUrl(resolved.world.ws_url.value)) {
    errors.push("world.ws_url must be a valid URL");
  }

  if (!isNonEmptyString(resolved.world.auth.city_agent_id.value)) {
    errors.push("world.auth.city_agent_id is required");
  }

  if (!isNonEmptyString(resolved.world.auth.bearer_token.value)) {
    errors.push("world.auth.bearer_token is required");
  }

  if (!isNonEmptyString(resolved.world.tool_manifest_strategy.value)) {
    errors.push("world.tool_manifest_strategy is required");
  }

  if (!["server_manifest", "static_fallback"].includes(resolved.world.tool_manifest_strategy.value)) {
    errors.push("world.tool_manifest_strategy must be one of server_manifest, static_fallback");
  }

  if (!isNonEmptyString(resolved.world.tool_auth.mode.value)) {
    errors.push("world.tool_auth.mode is required");
  }

  if (resolved.world.tool_auth.mode.value === "sim_key"
    && !isNonEmptyString(resolved.world.tool_auth.sim_api_key.value)) {
    errors.push("world.tool_auth.sim_api_key is required when tool auth mode is sim_key");
  }

  if (!isNonEmptyString(resolved.letta.agent_id.value)) {
    errors.push("letta.agent_id is required");
  }

  if (!isNonEmptyString(resolved.letta.api_key.value)) {
    errors.push("letta.api_key is required");
  }

  if (!isNonEmptyString(resolved.letta.base_url.value) || !isUrl(resolved.letta.base_url.value)) {
    errors.push("letta.base_url must be a valid URL");
  }

  if (!isPositiveInteger(resolved.runtime.max_wake_iterations.value)) {
    errors.push("runtime.max_wake_iterations must be a positive integer");
  }

  if (!isPositiveInteger(resolved.runtime.reconnect_initial_ms.value)) {
    errors.push("runtime.reconnect_initial_ms must be a positive integer");
  }

  if (!isPositiveInteger(resolved.runtime.reconnect_max_ms.value)) {
    errors.push("runtime.reconnect_max_ms must be a positive integer");
  }

  if (resolved.runtime.reconnect_max_ms.value < resolved.runtime.reconnect_initial_ms.value) {
    errors.push("runtime.reconnect_max_ms must be >= runtime.reconnect_initial_ms");
  }

  if (!isPositiveInteger(resolved.runtime.recent_wake_cache_size.value)) {
    errors.push("runtime.recent_wake_cache_size must be a positive integer");
  }

  if (!isPositiveInteger(resolved.runtime.action_timeout_ms.value)) {
    errors.push("runtime.action_timeout_ms must be a positive integer");
  }

  if (!isBoolean(resolved.runtime.wake_auto_done.value)) {
    errors.push("runtime.wake_auto_done must be a boolean");
  }

  if (!isBoolean(resolved.runtime.abort_on_unhandled_tool.value)) {
    errors.push("runtime.abort_on_unhandled_tool must be a boolean");
  }

  if (!isBoolean(resolved.runtime.allow_wake_replay_on_reconnect.value)) {
    errors.push("runtime.allow_wake_replay_on_reconnect must be a boolean");
  }

  if (!isPositiveInteger(resolved.runtime.max_consecutive_errors.value)) {
    errors.push("runtime.max_consecutive_errors must be a positive integer");
  }

  if (!isBoolean(resolved.runtime.pause_on_error.value)) {
    errors.push("runtime.pause_on_error must be a boolean");
  }

  if (!isNonEmptyString(resolved.runtime.log_level.value)) {
    errors.push("runtime.log_level is required");
  }

  if (!["plain", "tui", "interactive"].includes(resolved.ui.display_mode.value)) {
    errors.push("ui.display_mode must be one of plain, tui, interactive");
  }

  if (!isNonEmptyString(resolved.ui.theme.value)) {
    errors.push("ui.theme is required");
  }

  if (!isPositiveInteger(resolved.ui.refresh_ms.value)) {
    errors.push("ui.refresh_ms must be a positive integer");
  }

  if (!isPositiveInteger(resolved.ui.event_history_limit.value)) {
    errors.push("ui.event_history_limit must be a positive integer");
  }

  if (!isBoolean(resolved.ui.show_raw_events.value)) {
    errors.push("ui.show_raw_events must be a boolean");
  }

  if (resolved.mode === "interactive" && resolved.profile.source === "none") {
    warnings.push("interactive mode is running without a selected profile");
  }

  return {
    ok: errors.length === 0,
    errors,
    warnings,
  };
}
