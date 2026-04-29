#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir, homedir } from "node:os";
import path from "node:path";

function usage() {
  console.error(`Usage:
  lcity-agent.mjs [--repo <path>] [--api-base <url>] [--sim-key <key>] [--agent-id <id>|--agent-id-file <path>] <lcity-command> [args...]

Examples:
  lcity-agent.mjs --agent-id eddy_lin health_check
  lcity-agent.mjs --agent-id eddy_lin move_to --location-id lin_kitchen
  lcity-agent.mjs pathfind --from lin_bedroom --to hobbs_cafe_seating`);
}

function expandHome(value) {
  if (!value) return value;
  if (value === "~") return homedir();
  if (value.startsWith("~/")) return path.join(homedir(), value.slice(2));
  return value;
}

function defaultRepo() {
  if (process.env.LCITY_REPO) return expandHome(process.env.LCITY_REPO);
  if (existsSync(path.join(process.cwd(), "lcity", "bin", "lcity.mjs"))) return process.cwd();
  return path.join(homedir(), "letta", "letta-city-sim");
}

const tokens = process.argv.slice(2);
let repo = defaultRepo();
let apiBase = process.env.LCITY_API_BASE || "http://localhost:3001";
let simKey = process.env.SIM_API_KEY || "";
let agentId = "";
let agentIdFile = "";
const rest = [];

for (let i = 0; i < tokens.length; i += 1) {
  const token = tokens[i];
  if (token === "--repo") repo = expandHome(tokens[++i]);
  else if (token === "--api-base") apiBase = tokens[++i];
  else if (token === "--sim-key") simKey = tokens[++i];
  else if (token === "--agent-id") agentId = tokens[++i];
  else if (token === "--agent-id-file") agentIdFile = expandHome(tokens[++i]);
  else rest.push(token);
}

if (rest.length === 0) {
  usage();
  process.exit(2);
}

const lcityPath = path.join(repo, "lcity", "bin", "lcity.mjs");
if (!existsSync(lcityPath)) {
  console.error(JSON.stringify({ ok: false, error: `lcity not found at ${lcityPath}` }));
  process.exit(1);
}

if (agentId && !agentIdFile) {
  const dir = mkdtempSync(path.join(tmpdir(), "lcity-agent-"));
  agentIdFile = path.join(dir, "agent_id");
  writeFileSync(agentIdFile, `${agentId}\n`, "utf8");
}

const args = [lcityPath, "--api-base", apiBase];
if (simKey) args.push("--sim-key", simKey);
if (agentIdFile) args.push("--agent-id-file", agentIdFile);
args.push(...rest);

const result = spawnSync(process.execPath, args, {
  cwd: repo,
  stdio: "inherit",
  env: {
    ...process.env,
    LCITY_API_BASE: apiBase,
    SIM_API_KEY: simKey || process.env.SIM_API_KEY || "",
  },
});

process.exit(result.status ?? 1);
