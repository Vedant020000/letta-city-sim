import fs from "node:fs";
import path from "node:path";

function parseCli(argv) {
  const ctx = {
    apiBase: process.env.LCITY_API_BASE || "http://localhost:3001",
    agentIdFile: path.join(".lcity", "agent_id"),
  };

  const tokens = [...argv];
  while (tokens.length > 0 && tokens[0].startsWith("--")) {
    const token = tokens.shift();
    if (token === "--api-base") ctx.apiBase = tokens.shift() || ctx.apiBase;
    if (token === "--agent-id-file") ctx.agentIdFile = tokens.shift() || ctx.agentIdFile;
  }

  const command = tokens.shift() || null;
  const options = {};
  while (tokens.length > 0) {
    const token = tokens.shift();
    if (!token.startsWith("--")) continue;
    const key = token.slice(2);
    const value = tokens[0] && !tokens[0].startsWith("--") ? tokens.shift() : "true";
    options[key] = value;
  }

  return { ctx, command, options };
}

function required(options, key) {
  const value = options[key];
  if (!value || !String(value).trim()) {
    throw new Error(`missing --${key}`);
  }
  return String(value).trim();
}

function resolveAgentId(agentIdFile) {
  if (!fs.existsSync(agentIdFile)) {
    throw new Error("missing ./.lcity/agent_id (or pass --agent-id-file)");
  }
  try {
    const value = fs.readFileSync(agentIdFile, "utf8").trim();
    if (!value) throw new Error("agent_id file is empty");
    return value;
  } catch (err) {
    throw new Error(`failed to read agent id file: ${err.message}`);
  }
}

async function requestJson(url, { method = "GET", headers = {}, body } = {}) {
  const req = {
    method,
    headers: {
      ...headers,
    },
  };

  if (body !== undefined) {
    req.headers["Content-Type"] = "application/json";
    req.body = JSON.stringify(body);
  }

  try {
    const response = await fetch(url, req);
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

function okStatus(statusCode) {
  return statusCode >= 200 && statusCode < 300;
}

async function callApi(ctx, route, { method = "GET", body, requiresAgent = false } = {}) {
  const headers = {};
  if (requiresAgent) {
    headers["x-agent-id"] = resolveAgentId(ctx.agentIdFile);
  }

  const { statusCode, data } = await requestJson(`${ctx.apiBase.replace(/\/$/, "")}${route}`, {
    method,
    headers,
    body,
  });

  const output = { ok: okStatus(statusCode), status_code: statusCode, data };
  console.log(JSON.stringify(output));
  return output.ok ? 0 : 1;
}

function usage() {
  return [
    "lcity health_check",
    "lcity move_to --location-id lin_kitchen",
    "lcity move_to_agent --target-agent-id sam_moore",
    "lcity list_locations",
    "lcity get_location --id lin_kitchen",
    "lcity nearby_locations --id lin_kitchen",
    "lcity pathfind --from lin_bedroom --to hobbs_cafe_seating",
    "lcity world_time",
    "lcity list_inventory",
    "lcity board_read",
    "lcity board_posts",
    "lcity board_post --text \"Town hall at 6 PM\"",
    "lcity board_delete --post-id <id>",
    "lcity board_clear",
  ];
}

export async function run(argv) {
  try {
    const { ctx, command, options } = parseCli(argv);

    if (!command || command === "help" || command === "--help") {
      console.log(JSON.stringify({ ok: true, usage: usage() }));
      return 0;
    }

    switch (command) {
      case "health_check":
        return callApi(ctx, "/agents/health", { requiresAgent: true });
      case "move_to":
        return callApi(ctx, "/agents/move", {
          method: "PATCH",
          requiresAgent: true,
          body: { location_id: required(options, "location-id") },
        });
      case "move_to_agent": {
        const targetAgentId = required(options, "target-agent-id");
        const targetResp = await requestJson(
          `${ctx.apiBase.replace(/\/$/, "")}/agents/${encodeURIComponent(targetAgentId)}`,
        );
        if (!okStatus(targetResp.statusCode)) {
          console.log(
            JSON.stringify({
              ok: false,
              status_code: targetResp.statusCode,
              data: targetResp.data,
            }),
          );
          return 1;
        }

        const targetLocation = targetResp.data.current_location_id;
        if (!targetLocation) {
          console.log(
            JSON.stringify({ ok: false, error: "target agent has no current_location_id" }),
          );
          return 1;
        }

        return callApi(ctx, "/agents/move", {
          method: "PATCH",
          requiresAgent: true,
          body: { location_id: targetLocation },
        });
      }
      case "list_locations":
        return callApi(ctx, "/locations");
      case "get_location":
        return callApi(ctx, `/locations/${encodeURIComponent(required(options, "id"))}`);
      case "nearby_locations":
        return callApi(ctx, `/locations/${encodeURIComponent(required(options, "id"))}/nearby`);
      case "pathfind": {
        const from = encodeURIComponent(required(options, "from"));
        const to = encodeURIComponent(required(options, "to"));
        return callApi(ctx, `/pathfind?from=${from}&to=${to}`);
      }
      case "world_time":
        return callApi(ctx, "/world/time");
      case "list_inventory": {
        const agentId = resolveAgentId(ctx.agentIdFile);
        return callApi(ctx, `/inventory/${encodeURIComponent(agentId)}`);
      }
      case "board_read":
        return callApi(ctx, "/board");
      case "board_posts":
        return callApi(ctx, "/board/posts");
      case "board_post":
        return callApi(ctx, "/board/posts", {
          method: "PATCH",
          requiresAgent: true,
          body: { text: required(options, "text") },
        });
      case "board_delete":
        return callApi(ctx, `/board/posts/${encodeURIComponent(required(options, "post-id"))}`, {
          method: "DELETE",
          requiresAgent: true,
        });
      case "board_clear":
        return callApi(ctx, "/board/clear", { method: "DELETE", requiresAgent: true });
      default:
        console.log(JSON.stringify({ ok: false, error: `unknown command: ${command}` }));
        return 1;
    }
  } catch (err) {
    console.log(JSON.stringify({ ok: false, error: err.message }));
    return 1;
  }
}
