import { resolveRuntimeConfig } from "../config.mjs";

function printJson(value) {
  console.log(JSON.stringify(value, null, 2));
}

export async function runToolsCommand({ subcommand, flags }) {
  const resolved = resolveRuntimeConfig({ flags, cwd: flags.cwd || process.cwd() });

  switch (subcommand || "preview") {
    case "preview": {
      const agentId = resolved.world.auth.city_agent_id.value;
      if (!agentId) {
        throw new Error("missing city agent id; set .lcity/agent_id, profile world.auth.city_agent_id, or --city-agent-id");
      }

      const response = await fetch(`${resolved.world.api_base.value}/agents/${encodeURIComponent(agentId)}/tool-manifest`, {
        method: "GET",
        headers: {
          "x-agent-id": agentId,
        },
      });

      const text = await response.text();
      let body;
      try {
        body = text ? JSON.parse(text) : null;
      } catch {
        body = { raw: text };
      }

      printJson({
        ok: response.ok,
        status: response.status,
        manifest: body,
      });
      return response.ok ? 0 : 1;
    }
    default:
      throw new Error(`unknown tools subcommand: ${subcommand}`);
  }
}
