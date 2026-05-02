import fs from "node:fs";
import path from "node:path";
import {
  ACTIVE_PROFILE_FILE,
  LEGACY_ACTIVE_PROFILE_FILE,
  LEGACY_PROFILE_DIR,
  LEGACY_STATE_DIR,
  PROFILE_DIR,
  STATE_DIR,
} from "./schema.mjs";

function ensureDir(dirPath) {
  fs.mkdirSync(dirPath, { recursive: true });
}

function normalizeName(name) {
  const normalized = String(name || "").trim();
  if (!normalized) {
    throw new Error("profile name is required");
  }

  if (!/^[a-zA-Z0-9._-]+$/.test(normalized)) {
    throw new Error("profile name may only contain letters, numbers, dot, underscore, and dash");
  }

  return normalized;
}

function readOptionalFile(filePath) {
  try {
    if (!fs.existsSync(filePath)) return "";
    return fs.readFileSync(filePath, "utf8").trim();
  } catch {
    return "";
  }
}

export function getProfileDir(cwd = process.cwd()) {
  return path.join(cwd, PROFILE_DIR.replace(/\//g, path.sep));
}

export function getStateDir(cwd = process.cwd()) {
  return path.join(cwd, STATE_DIR.replace(/\//g, path.sep));
}

function getLegacyProfileDir(cwd = process.cwd()) {
  return path.join(cwd, LEGACY_PROFILE_DIR.replace(/\//g, path.sep));
}

function getLegacyStateDir(cwd = process.cwd()) {
  return path.join(cwd, LEGACY_STATE_DIR.replace(/\//g, path.sep));
}

function getLegacyProfilePath(name, cwd = process.cwd()) {
  return path.join(getLegacyProfileDir(cwd), `${normalizeName(name)}.json`);
}

export function getProfilePath(name, cwd = process.cwd()) {
  return path.join(getProfileDir(cwd), `${normalizeName(name)}.json`);
}

export function listProfiles(cwd = process.cwd()) {
  const names = new Set();

  for (const profileDir of [getProfileDir(cwd), getLegacyProfileDir(cwd)]) {
    if (!fs.existsSync(profileDir)) continue;

    for (const entry of fs.readdirSync(profileDir)) {
      if (entry.endsWith(".json")) {
        names.add(entry.slice(0, -5));
      }
    }
  }

  return [...names].sort();
}

export function readProfile(name, cwd = process.cwd()) {
  const profilePath = fs.existsSync(getProfilePath(name, cwd))
    ? getProfilePath(name, cwd)
    : getLegacyProfilePath(name, cwd);

  if (!fs.existsSync(profilePath)) {
    throw new Error(`profile not found: ${name}`);
  }

  const raw = fs.readFileSync(profilePath, "utf8");
  try {
    return JSON.parse(raw);
  } catch (error) {
    throw new Error(`profile is not valid JSON: ${name} (${error.message})`);
  }
}

export function writeProfile(name, data, cwd = process.cwd()) {
  ensureDir(getProfileDir(cwd));
  const profilePath = getProfilePath(name, cwd);
  fs.writeFileSync(profilePath, `${JSON.stringify(data, null, 2)}\n`, "utf8");
  return profilePath;
}

export function deleteProfile(name, cwd = process.cwd()) {
  for (const profilePath of [getProfilePath(name, cwd), getLegacyProfilePath(name, cwd)]) {
    if (fs.existsSync(profilePath)) {
      fs.unlinkSync(profilePath);
    }
  }
}

export function setActiveProfile(name, cwd = process.cwd()) {
  ensureDir(getStateDir(cwd));
  const activePath = path.join(cwd, ACTIVE_PROFILE_FILE.replace(/\//g, path.sep));
  fs.writeFileSync(activePath, `${normalizeName(name)}\n`, "utf8");
}

export function getActiveProfile(cwd = process.cwd()) {
  const activePath = path.join(cwd, ACTIVE_PROFILE_FILE.replace(/\//g, path.sep));
  const legacyActivePath = path.join(cwd, LEGACY_ACTIVE_PROFILE_FILE.replace(/\//g, path.sep));
  const value = readOptionalFile(activePath) || readOptionalFile(legacyActivePath);
  return value || "";
}

export function hasProfile(name, cwd = process.cwd()) {
  return fs.existsSync(getProfilePath(name, cwd)) || fs.existsSync(getLegacyProfilePath(name, cwd));
}

export function buildBootstrapProfile({ cwd = process.cwd(), name = "default" } = {}) {
  const profile = {
    world: {},
    letta: {},
    runtime: {},
    ui: {},
  };

  const apiBasePath = path.join(cwd, ".lcity", "api_base");
  const agentIdPath = path.join(cwd, ".lcity", "agent_id");
  const tokenPath = path.join(cwd, ".lcity", "agent_token");

  const apiBase = readOptionalFile(apiBasePath);
  const cityAgentId = readOptionalFile(agentIdPath);

  if (apiBase) {
    profile.world.api_base = apiBase;
  }

  profile.world.tool_manifest_strategy = "server_manifest";
  profile.world.tool_auth = {
    mode: "sim_key",
    sim_api_key_source: process.env.SIM_API_KEY ? "env:SIM_API_KEY" : "env:SIM_API_KEY",
  };

  profile.world.auth = {};
  if (cityAgentId) {
    profile.world.auth.city_agent_id = cityAgentId;
  }

  if (fs.existsSync(tokenPath)) {
    profile.world.auth.bearer_token_source = "file:.lcity/agent_token";
  } else if (process.env.LCITY_AGENT_TOKEN) {
    profile.world.auth.bearer_token_source = "env:LCITY_AGENT_TOKEN";
  }

  if (process.env.LETTA_AGENT_ID) {
    profile.letta.agent_id = process.env.LETTA_AGENT_ID;
  }

  profile.letta.api_key_source = process.env.LETTA_API_KEY ? "env:LETTA_API_KEY" : "env:LETTA_API_KEY";
  profile.ui.display_mode = "tui";
  profile.ui.theme = "midnight";

  return {
    name: normalizeName(name),
    profile,
  };
}
