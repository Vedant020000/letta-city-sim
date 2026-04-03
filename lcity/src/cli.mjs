import fs from "node:fs";
import path from "node:path";

function parseArgs(argv) {
  const args = {
    command: null,
    apiBase: process.env.LCITY_API_BASE || "http://localhost:3001",
    agentIdFile: path.join(".lcity", "agent_id"),
  };

  const tokens = [...argv];
  while (tokens.length > 0) {
    const token = tokens.shift();

    if (!args.command && !token.startsWith("--")) {
      args.command = token;
      continue;
    }

    if (token === "--api-base") {
      args.apiBase = tokens.shift() || args.apiBase;
      continue;
    }

    if (token === "--agent-id-file") {
      args.agentIdFile = tokens.shift() || args.agentIdFile;
      continue;
    }
  }

  return args;
}

function resolveAgentId(agentIdFile) {
  if (fs.existsSync(agentIdFile)) {
    try {
      const fileAgentId = fs.readFileSync(agentIdFile, "utf8").trim();
      if (fileAgentId) {
        return { agentId: fileAgentId, error: null };
      }
    } catch (err) {
      return { agentId: null, error: `failed to read agent id file: ${err.message}` };
    }
  }

  return {
    agentId: null,
    error: "missing ./.lcity/agent_id (or pass --agent-id-file)",
  };
}

async function requestJson(url, { method = "GET", headers = {} } = {}) {
  try {
    const response = await fetch(url, { method, headers });
    const text = await response.text();
    let data = {};
    if (text) {
      try {
        data = JSON.parse(text);
      } catch {
        data = { error: text };
      }
    }

    return { statusCode: response.status, data };
  } catch (err) {
    return { statusCode: 0, data: { error: `network error: ${err.message}` } };
  }
}

async function healthCheck(apiBase, agentId) {
  const { statusCode, data } = await requestJson(`${apiBase.replace(/\/$/, "")}/agents/health`, {
    method: "GET",
    headers: {
      "x-agent-id": agentId,
    },
  });

  const output = {
    ok: statusCode === 200,
    status_code: statusCode,
    data,
  };

  console.log(JSON.stringify(output));
  return statusCode === 200 ? 0 : 1;
}

export async function run(argv) {
  const args = parseArgs(argv);

  if (!args.command || args.command === "help" || args.command === "--help") {
    console.log(
      JSON.stringify({
        ok: true,
        usage: [
          "lcity health_check",
          "lcity --api-base http://localhost:3001 health_check",
          "lcity health_check --agent-id-file .lcity/agent_id",
        ],
      }),
    );
    return 0;
  }

  if (args.command === "health_check") {
    const { agentId, error } = resolveAgentId(args.agentIdFile);
    if (error) {
      console.log(JSON.stringify({ ok: false, error }));
      return 1;
    }

    return healthCheck(args.apiBase, agentId);
  }

  console.log(JSON.stringify({ ok: false, error: `unknown command: ${args.command}` }));
  return 1;
}
