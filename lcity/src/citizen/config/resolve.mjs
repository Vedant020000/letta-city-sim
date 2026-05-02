import fs from "node:fs";
import path from "node:path";
import {
  ACTIVE_PROFILE_FILE,
  DEFAULTS,
  LEGACY_ACTIVE_PROFILE_FILE,
  createResolvedValue,
  deepClone,
} from "./schema.mjs";
import { getActiveProfile, readProfile } from "./profile-store.mjs";

function readOptionalFile(filePath) {
  try {
    if (!fs.existsSync(filePath)) return "";
    return fs.readFileSync(filePath, "utf8").trim();
  } catch {
    return "";
  }
}

function normalizeApiBase(apiBase) {
  return String(apiBase || DEFAULTS.world.api_base).trim().replace(/\/$/, "");
}

function wsUrlFromApiBase(apiBase) {
  const trimmed = normalizeApiBase(apiBase);
  if (trimmed.startsWith("https://")) return `wss://${trimmed.slice("https://".length)}/ws/citizen`;
  if (trimmed.startsWith("http://")) return `ws://${trimmed.slice("http://".length)}/ws/citizen`;
  return `${trimmed}/ws/citizen`;
}

function parsePositiveInteger(value, fallback) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed <= 0) return fallback;
  return Math.floor(parsed);
}

function parseBoolean(value, fallback) {
  if (typeof value === "boolean") return value;
  if (typeof value === "string") {
    const normalized = value.trim().toLowerCase();
    if (normalized === "true") return true;
    if (normalized === "false") return false;
  }
  return fallback;
}

function isNonEmptyString(value) {
  return typeof value === "string" && value.trim().length > 0;
}

function resolveSecretFromSource(reference, cwd) {
  if (!isNonEmptyString(reference)) {
    return createResolvedValue({ value: "", source: "missing", masked: true, present: false });
  }

  if (reference.startsWith("env:")) {
    const envName = reference.slice(4);
    const value = process.env[envName] ? String(process.env[envName]).trim() : "";
    return createResolvedValue({
      value,
      source: reference,
      masked: true,
      present: Boolean(value),
    });
  }

  if (reference.startsWith("file:")) {
    const filePath = reference.slice(5);
    const resolvedPath = path.isAbsolute(filePath) ? filePath : path.join(cwd, filePath.replace(/\//g, path.sep));
    const value = readOptionalFile(resolvedPath);
    return createResolvedValue({
      value,
      source: `file:${filePath.replace(/\\/g, "/")}`,
      masked: true,
      present: Boolean(value),
    });
  }

  return createResolvedValue({
    value: reference,
    source: "inline",
    masked: true,
    present: Boolean(reference),
  });
}

function determineMode(flags, cwd) {
  const explicit = isNonEmptyString(flags.mode) ? flags.mode.trim().toLowerCase() : "";
  if (explicit === "env" || explicit === "interactive") {
    return createResolvedValue({ value: explicit, source: "flag:--mode" });
  }

  const envValue = process.env.LCITY_CITIZEN_MODE ? String(process.env.LCITY_CITIZEN_MODE).trim().toLowerCase() : "";
  if (envValue === "env" || envValue === "interactive") {
    return createResolvedValue({ value: envValue, source: "env:LCITY_CITIZEN_MODE" });
  }

  if (flags.command === "interactive") {
    return createResolvedValue({ value: "interactive", source: "command:interactive" });
  }

  const activeProfile = getActiveProfile(cwd);
  if (activeProfile && (flags.profile || flags.command === "profile" || process.stdout.isTTY)) {
    return createResolvedValue({ value: "interactive", source: "state:active_profile" });
  }

  return createResolvedValue({ value: "env", source: "default" });
}

function determineProfile(flags, modeEntry, cwd) {
  if (isNonEmptyString(flags.profile)) {
    return createResolvedValue({ value: flags.profile.trim(), source: "flag:--profile" });
  }

  const activeProfile = getActiveProfile(cwd);
  if (activeProfile) {
    return createResolvedValue({ value: activeProfile, source: "state:active_profile" });
  }

  if (modeEntry.value === "interactive") {
    return createResolvedValue({ value: "", source: "none", present: false });
  }

  return createResolvedValue({ value: "", source: "none", present: false });
}

function loadProfileIfAny(profileEntry, cwd) {
  if (!profileEntry.present || !profileEntry.value) {
    return { value: null, source: "none" };
  }

  return {
    value: readProfile(profileEntry.value, cwd),
    source: `profile:${profileEntry.value}`,
  };
}

function resolveDisplayMode(flags, modeEntry, profile) {
  if (flags.plain) {
    return createResolvedValue({ value: "plain", source: "flag:--plain" });
  }

  if (flags.tui) {
    return createResolvedValue({ value: "tui", source: "flag:--tui" });
  }

  if (profile?.ui?.display_mode && profile.ui.display_mode !== "auto") {
    return createResolvedValue({ value: profile.ui.display_mode, source: "profile:ui.display_mode" });
  }

  if (modeEntry.value === "interactive") {
    return createResolvedValue({ value: "interactive", source: `derived:${modeEntry.source}` });
  }

  return createResolvedValue({ value: "plain", source: `derived:${modeEntry.source}` });
}

function resolvedString({ flagValue, envName, profileValue, filePath, defaultValue, label, cwd, mask = false }) {
  if (isNonEmptyString(flagValue)) {
    return createResolvedValue({ value: String(flagValue).trim(), source: `flag:${label}`, masked: mask });
  }

  if (isNonEmptyString(envName) && isNonEmptyString(process.env[envName])) {
    return createResolvedValue({ value: String(process.env[envName]).trim(), source: `env:${envName}`, masked: mask });
  }

  if (isNonEmptyString(profileValue)) {
    return createResolvedValue({ value: String(profileValue).trim(), source: `profile:${label}`, masked: mask });
  }

  if (isNonEmptyString(filePath)) {
    const resolvedPath = path.isAbsolute(filePath) ? filePath : path.join(cwd, filePath.replace(/\//g, path.sep));
    const value = readOptionalFile(resolvedPath);
    if (value) {
      return createResolvedValue({
        value,
        source: `file:${filePath.replace(/\\/g, "/")}`,
        masked: mask,
      });
    }
  }

  return createResolvedValue({ value: defaultValue, source: "default", masked: mask, present: defaultValue !== "" });
}

function resolvedNumber({ flagValue, envName, profileValue, defaultValue, label }) {
  if (flagValue != null) {
    return createResolvedValue({ value: parsePositiveInteger(flagValue, defaultValue), source: `flag:${label}` });
  }

  if (isNonEmptyString(envName) && process.env[envName]) {
    return createResolvedValue({ value: parsePositiveInteger(process.env[envName], defaultValue), source: `env:${envName}` });
  }

  if (profileValue != null) {
    return createResolvedValue({ value: parsePositiveInteger(profileValue, defaultValue), source: `profile:${label}` });
  }

  return createResolvedValue({ value: defaultValue, source: "default" });
}

function resolvedBoolean({ envName, profileValue, defaultValue, label }) {
  if (isNonEmptyString(envName) && process.env[envName]) {
    return createResolvedValue({ value: parseBoolean(process.env[envName], defaultValue), source: `env:${envName}` });
  }

  if (profileValue != null) {
    return createResolvedValue({ value: parseBoolean(profileValue, defaultValue), source: `profile:${label}` });
  }

  return createResolvedValue({ value: defaultValue, source: "default" });
}

export function resolveRuntimeConfig({ flags = {}, cwd = process.cwd() } = {}) {
  const mode = determineMode(flags, cwd);
  const profile = determineProfile(flags, mode, cwd);
  const loadedProfile = loadProfileIfAny(profile, cwd);
  const profileData = loadedProfile.value || deepClone({});

  const apiBase = resolvedString({
    flagValue: flags.apiBase,
    envName: "LCITY_API_BASE",
    profileValue: profileData.world?.api_base,
    filePath: ".lcity/api_base",
    defaultValue: DEFAULTS.world.api_base,
    label: "--api-base",
    cwd,
  });

  const worldAuthProfile = profileData.world?.auth || {};
  const worldToolAuthProfile = profileData.world?.tool_auth || {};

  const cityAgentId = resolvedString({
    flagValue: flags.cityAgentId,
    envName: "LCITY_CITY_AGENT_ID",
    profileValue: worldAuthProfile.city_agent_id,
    filePath: flags.agentIdFile || ".lcity/agent_id",
    defaultValue: "",
    label: "--city-agent-id",
    cwd,
  });

  let bearerToken;
  if (isNonEmptyString(flags.agentToken)) {
    bearerToken = createResolvedValue({
      value: String(flags.agentToken).trim(),
      source: "flag:--agent-token",
      masked: true,
    });
  } else if (isNonEmptyString(process.env.LCITY_AGENT_TOKEN)) {
    bearerToken = createResolvedValue({
      value: String(process.env.LCITY_AGENT_TOKEN).trim(),
      source: "env:LCITY_AGENT_TOKEN",
      masked: true,
    });
  } else if (isNonEmptyString(worldAuthProfile.bearer_token_source)) {
    bearerToken = resolveSecretFromSource(worldAuthProfile.bearer_token_source, cwd);
  } else {
    const filePath = flags.agentTokenFile || ".lcity/agent_token";
    bearerToken = resolveSecretFromSource(`file:${filePath.replace(/\\/g, "/")}`, cwd);
  }

  let simApiKey;
  if (isNonEmptyString(flags.simKey)) {
    simApiKey = createResolvedValue({
      value: String(flags.simKey).trim(),
      source: "flag:--sim-key",
      masked: true,
    });
  } else if (isNonEmptyString(process.env.SIM_API_KEY)) {
    simApiKey = createResolvedValue({
      value: String(process.env.SIM_API_KEY).trim(),
      source: "env:SIM_API_KEY",
      masked: true,
    });
  } else if (isNonEmptyString(worldToolAuthProfile.sim_api_key_source)) {
    simApiKey = resolveSecretFromSource(worldToolAuthProfile.sim_api_key_source, cwd);
  } else {
    simApiKey = createResolvedValue({ value: "", source: "missing", masked: true, present: false });
  }

  const lettaProfile = profileData.letta || {};

  let lettaApiKey;
  if (isNonEmptyString(flags.lettaApiKey)) {
    lettaApiKey = createResolvedValue({
      value: String(flags.lettaApiKey).trim(),
      source: "flag:--letta-api-key",
      masked: true,
    });
  } else if (isNonEmptyString(process.env.LETTA_API_KEY)) {
    lettaApiKey = createResolvedValue({
      value: String(process.env.LETTA_API_KEY).trim(),
      source: "env:LETTA_API_KEY",
      masked: true,
    });
  } else if (isNonEmptyString(lettaProfile.api_key_source)) {
    lettaApiKey = resolveSecretFromSource(lettaProfile.api_key_source, cwd);
  } else {
    lettaApiKey = createResolvedValue({ value: "", source: "missing", masked: true, present: false });
  }

  const lettaAgentId = resolvedString({
    flagValue: flags.lettaAgentId,
    envName: "LETTA_AGENT_ID",
    profileValue: lettaProfile.agent_id,
    filePath: "",
    defaultValue: "",
    label: "--letta-agent-id",
    cwd,
  });

  const lettaBaseUrl = resolvedString({
    flagValue: flags.lettaBaseUrl,
    envName: "LETTA_BASE_URL",
    profileValue: lettaProfile.base_url,
    filePath: "",
    defaultValue: DEFAULTS.letta.base_url,
    label: "--letta-base-url",
    cwd,
  });

  const displayMode = resolveDisplayMode(flags, mode, profileData);

  const resolved = {
    mode: mode.value,
    mode_entry: mode,
    profile: {
      name: profile.value,
      source: profile.source,
      present: profile.present,
    },
    world: {
      api_base: apiBase,
      ws_url: createResolvedValue({ value: wsUrlFromApiBase(apiBase.value), source: `derived:${apiBase.source}` }),
      tool_manifest_strategy: resolvedString({
        flagValue: flags.toolManifestStrategy,
        envName: "LCITY_CITIZEN_TOOL_MANIFEST_STRATEGY",
        profileValue: profileData.world?.tool_manifest_strategy,
        filePath: "",
        defaultValue: DEFAULTS.world.tool_manifest_strategy,
        label: "world.tool_manifest_strategy",
        cwd,
      }),
      tool_auth: {
        mode: resolvedString({
          flagValue: flags.toolAuthMode,
          envName: "LCITY_CITIZEN_TOOL_AUTH_MODE",
          profileValue: worldToolAuthProfile.mode,
          filePath: "",
          defaultValue: DEFAULTS.world.tool_auth.mode,
          label: "world.tool_auth.mode",
          cwd,
        }),
        sim_api_key: simApiKey,
      },
      auth: {
        city_agent_id: cityAgentId,
        bearer_token: bearerToken,
      },
    },
    letta: {
      api_key: lettaApiKey,
      agent_id: lettaAgentId,
      base_url: lettaBaseUrl,
    },
    runtime: {
      max_wake_iterations: resolvedNumber({ flagValue: flags.maxWakeIterations, envName: "LCITY_CITIZEN_MAX_WAKE_ITERATIONS", profileValue: profileData.runtime?.max_wake_iterations, defaultValue: DEFAULTS.runtime.max_wake_iterations, label: "runtime.max_wake_iterations" }),
      reconnect_initial_ms: resolvedNumber({ flagValue: flags.reconnectInitialMs, envName: "LCITY_CITIZEN_RECONNECT_INITIAL_MS", profileValue: profileData.runtime?.reconnect_initial_ms, defaultValue: DEFAULTS.runtime.reconnect_initial_ms, label: "runtime.reconnect_initial_ms" }),
      reconnect_max_ms: resolvedNumber({ flagValue: flags.reconnectMaxMs, envName: "LCITY_CITIZEN_RECONNECT_MAX_MS", profileValue: profileData.runtime?.reconnect_max_ms, defaultValue: DEFAULTS.runtime.reconnect_max_ms, label: "runtime.reconnect_max_ms" }),
      recent_wake_cache_size: resolvedNumber({ flagValue: flags.recentWakeCacheSize, envName: "LCITY_CITIZEN_RECENT_WAKE_CACHE_SIZE", profileValue: profileData.runtime?.recent_wake_cache_size, defaultValue: DEFAULTS.runtime.recent_wake_cache_size, label: "runtime.recent_wake_cache_size" }),
      action_timeout_ms: resolvedNumber({ flagValue: flags.actionTimeoutMs, envName: "LCITY_CITIZEN_ACTION_TIMEOUT_MS", profileValue: profileData.runtime?.action_timeout_ms, defaultValue: DEFAULTS.runtime.action_timeout_ms, label: "runtime.action_timeout_ms" }),
      wake_auto_done: resolvedBoolean({ envName: "LCITY_CITIZEN_WAKE_AUTO_DONE", profileValue: profileData.runtime?.wake_auto_done, defaultValue: DEFAULTS.runtime.wake_auto_done, label: "runtime.wake_auto_done" }),
      abort_on_unhandled_tool: resolvedBoolean({ envName: "LCITY_CITIZEN_ABORT_ON_UNHANDLED_TOOL", profileValue: profileData.runtime?.abort_on_unhandled_tool, defaultValue: DEFAULTS.runtime.abort_on_unhandled_tool, label: "runtime.abort_on_unhandled_tool" }),
      allow_wake_replay_on_reconnect: resolvedBoolean({ envName: "LCITY_CITIZEN_ALLOW_WAKE_REPLAY_ON_RECONNECT", profileValue: profileData.runtime?.allow_wake_replay_on_reconnect, defaultValue: DEFAULTS.runtime.allow_wake_replay_on_reconnect, label: "runtime.allow_wake_replay_on_reconnect" }),
      max_consecutive_errors: resolvedNumber({ flagValue: flags.maxConsecutiveErrors, envName: "LCITY_CITIZEN_MAX_CONSECUTIVE_ERRORS", profileValue: profileData.runtime?.max_consecutive_errors, defaultValue: DEFAULTS.runtime.max_consecutive_errors, label: "runtime.max_consecutive_errors" }),
      pause_on_error: resolvedBoolean({ envName: "LCITY_CITIZEN_PAUSE_ON_ERROR", profileValue: profileData.runtime?.pause_on_error, defaultValue: DEFAULTS.runtime.pause_on_error, label: "runtime.pause_on_error" }),
      log_level: resolvedString({ flagValue: flags.logLevel, envName: "LCITY_CITIZEN_LOG_LEVEL", profileValue: profileData.runtime?.log_level, filePath: "", defaultValue: DEFAULTS.runtime.log_level, label: "runtime.log_level", cwd }),
    },
    ui: {
      display_mode: displayMode,
      theme: resolvedString({ flagValue: flags.theme, envName: "LCITY_CITIZEN_THEME", profileValue: profileData.ui?.theme, filePath: "", defaultValue: DEFAULTS.ui.theme, label: "ui.theme", cwd }),
      refresh_ms: resolvedNumber({ flagValue: flags.refreshMs, envName: "LCITY_CITIZEN_REFRESH_MS", profileValue: profileData.ui?.refresh_ms, defaultValue: DEFAULTS.ui.refresh_ms, label: "ui.refresh_ms" }),
      event_history_limit: resolvedNumber({ flagValue: flags.eventHistoryLimit, envName: "LCITY_CITIZEN_EVENT_HISTORY_LIMIT", profileValue: profileData.ui?.event_history_limit, defaultValue: DEFAULTS.ui.event_history_limit, label: "ui.event_history_limit" }),
      show_raw_events: resolvedBoolean({ envName: "LCITY_CITIZEN_SHOW_RAW_EVENTS", profileValue: profileData.ui?.show_raw_events, defaultValue: DEFAULTS.ui.show_raw_events, label: "ui.show_raw_events" }),
    },
    _profile_data: profileData,
  };

  return resolved;
}

export function configOverview(resolved) {
  return [
    { label: "Mode", value: resolved.mode, source: resolved.mode_entry.source },
    { label: "Profile", value: resolved.profile.present ? resolved.profile.name : "none", source: resolved.profile.source },
    { label: "World API", value: resolved.world.api_base.value, source: resolved.world.api_base.source },
    { label: "Citizen WS", value: resolved.world.ws_url.value, source: resolved.world.ws_url.source },
    { label: "Manifest", value: resolved.world.tool_manifest_strategy.value, source: resolved.world.tool_manifest_strategy.source },
    { label: "City agent", value: resolved.world.auth.city_agent_id.value || "missing", source: resolved.world.auth.city_agent_id.source },
    { label: "City token", value: resolved.world.auth.bearer_token.present ? "present" : "missing", source: resolved.world.auth.bearer_token.source },
    { label: "Sim key", value: resolved.world.tool_auth.sim_api_key.present ? "present" : "missing", source: resolved.world.tool_auth.sim_api_key.source },
    { label: "Letta base", value: resolved.letta.base_url.value, source: resolved.letta.base_url.source },
    { label: "Letta key", value: resolved.letta.api_key.present ? "present" : "missing", source: resolved.letta.api_key.source },
    { label: "Letta agent", value: resolved.letta.agent_id.value || "missing", source: resolved.letta.agent_id.source },
    { label: "Wake cap", value: String(resolved.runtime.max_wake_iterations.value), source: resolved.runtime.max_wake_iterations.source },
    { label: "Display", value: resolved.ui.display_mode.value, source: resolved.ui.display_mode.source },
  ];
}

function sanitizeNode(node) {
  if (Array.isArray(node)) {
    return node.map(sanitizeNode);
  }

  if (!node || typeof node !== "object") {
    return node;
  }

  if (Object.prototype.hasOwnProperty.call(node, "value") && Object.prototype.hasOwnProperty.call(node, "source")) {
    if (node.masked) {
      return {
        ...node,
        value: node.present ? "present" : "missing",
      };
    }
    return { ...node };
  }

  const output = {};
  for (const [key, value] of Object.entries(node)) {
    output[key] = sanitizeNode(value);
  }
  return output;
}

export function flattenResolvedConfig(resolved, { sanitize = true } = {}) {
  const flattened = {
    mode: { value: resolved.mode, source: resolved.mode_entry.source },
    profile: resolved.profile,
    world: resolved.world,
    letta: resolved.letta,
    runtime: resolved.runtime,
    ui: resolved.ui,
  };

  return sanitize ? sanitizeNode(flattened) : flattened;
}

export function activeProfilePath(cwd = process.cwd()) {
  const canonical = path.join(cwd, ACTIVE_PROFILE_FILE.replace(/\//g, path.sep));
  if (fs.existsSync(canonical)) return canonical;
  return path.join(cwd, LEGACY_ACTIVE_PROFILE_FILE.replace(/\//g, path.sep));
}
