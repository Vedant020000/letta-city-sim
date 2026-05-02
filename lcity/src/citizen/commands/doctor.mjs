import fs from "node:fs";
import path from "node:path";
import { flattenResolvedConfig, resolveRuntimeConfig, validateResolvedConfig } from "../config.mjs";

function printJson(value) {
  console.log(JSON.stringify(value, null, 2));
}

export async function runDoctorCommand({ flags }) {
  const cwd = flags.cwd || process.cwd();
  const resolved = resolveRuntimeConfig({ flags, cwd });
  const validation = validateResolvedConfig(resolved);
  const checks = [];

  const agentIdPath = path.join(cwd, ".lcity", "agent_id");
  const agentTokenPath = path.join(cwd, ".lcity", "agent_token");
  const apiBasePath = path.join(cwd, ".lcity", "api_base");

  checks.push({ name: ".lcity/agent_id", ok: fs.existsSync(agentIdPath), detail: agentIdPath });
  checks.push({ name: ".lcity/agent_token", ok: fs.existsSync(agentTokenPath), detail: agentTokenPath });
  checks.push({ name: ".lcity/api_base", ok: fs.existsSync(apiBasePath), detail: apiBasePath });
  checks.push({ name: "Letta API key present", ok: resolved.letta.api_key.present, detail: resolved.letta.api_key.source });
  checks.push({ name: "Letta agent id present", ok: Boolean(resolved.letta.agent_id.value), detail: resolved.letta.agent_id.source });
  checks.push({ name: "SIM_API_KEY present", ok: resolved.world.tool_auth.sim_api_key.present, detail: resolved.world.tool_auth.sim_api_key.source });
  checks.push({ name: "Tool manifest strategy", ok: Boolean(resolved.world.tool_manifest_strategy.value), detail: resolved.world.tool_manifest_strategy.value });
  checks.push({ name: "World API base", ok: Boolean(resolved.world.api_base.value), detail: resolved.world.api_base.value });
  checks.push({ name: "Citizen WS URL", ok: Boolean(resolved.world.ws_url.value), detail: resolved.world.ws_url.value });

  printJson({
    ok: validation.ok,
    checks,
    validation,
    config: flattenResolvedConfig(resolved),
  });

  return validation.ok ? 0 : 1;
}
