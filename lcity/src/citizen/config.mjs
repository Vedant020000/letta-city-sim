import path from "node:path";
import { buildBootstrapProfile, deleteProfile, getActiveProfile, getProfilePath, hasProfile, listProfiles, readProfile, setActiveProfile, writeProfile } from "./config/profile-store.mjs";
import { configOverview, flattenResolvedConfig, resolveRuntimeConfig } from "./config/resolve.mjs";
import { validateResolvedConfig } from "./config/validate.mjs";

function readFlagValue(tokens, fallback) {
  const next = tokens[0];
  if (!next || next.startsWith("--")) return fallback;
  return tokens.shift();
}

export function usage() {
  return [
    "lcity citizen run [--mode env|interactive] [--plain|--tui] [--profile <name>]",
    "lcity citizen config show [--profile <name>]",
    "lcity citizen config validate [--profile <name>]",
    "lcity citizen profile list",
    "lcity citizen profile init --name <profile>",
    "lcity citizen profile use --name <profile>",
    "lcity citizen doctor [--profile <name>]",
    "lcity citizen tools preview [--profile <name>]",
    "Key flags: --api-base, --ws-url, --city-agent-id, --agent-token, --sim-key, --tool-manifest-strategy, --tool-auth-mode",
  ];
}

export function parseCli(argv) {
  const flags = {
    agentIdFile: path.join(".lcity", "agent_id"),
    agentTokenFile: path.join(".lcity", "agent_token"),
    cwd: process.cwd(),
  };

  const positional = [];
  const tokens = [...argv];

  while (tokens.length > 0) {
    const token = tokens.shift();
    if (!token.startsWith("--")) {
      positional.push(token);
      continue;
    }

    const key = token.slice(2);
    switch (key) {
      case "help":
        flags.help = true;
        break;
      case "plain":
        flags.plain = true;
        break;
      case "tui":
        flags.tui = true;
        break;
      default:
        flags[key.replace(/-([a-z])/g, (_, ch) => ch.toUpperCase())] = readFlagValue(tokens, true);
        break;
    }
  }

  const command = positional[0] || "run";
  const subcommand = positional[1] || "";

  return {
    command,
    subcommand,
    positional,
    flags: {
      ...flags,
      command,
      subcommand,
    },
  };
}

export {
  buildBootstrapProfile,
  configOverview,
  deleteProfile,
  flattenResolvedConfig,
  getActiveProfile,
  getProfilePath,
  hasProfile,
  listProfiles,
  readProfile,
  resolveRuntimeConfig,
  setActiveProfile,
  validateResolvedConfig,
  writeProfile,
};
