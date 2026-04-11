import fs from "node:fs";
import path from "node:path";
import os from "node:os";
import http from "node:http";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

async function postLettaBotCompletion({ lettabotBase, lettabotKey, agentId, messages }) {
  const body = {
    agent_id: agentId,
    messages,
  };

  return fetch(`${lettabotBase.replace(/\/$/, "")}/v1/chat/completions`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${lettabotKey}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify(body),
  });
}

async function wakeAgentViaLettaBot({ lettabotBase, lettabotKey, agentId, envelope }) {
  // NOTE: Vedant confirmed LettaBot /v1/chat/completions is a persistent agent call.
  // We keep payload minimal and deterministic; tools/agent logic should interpret the event.
  const messages = [
    {
      role: "user",
      content: JSON.stringify({
        kind: "world_event",
        event: envelope,
      }),
    },
  ];

  return postLettaBotCompletion({ lettabotBase, lettabotKey, agentId, messages });
}

async function sendLettaBotMessage({ lettabotBase, lettabotKey, agentId, message }) {
  const trimmed = String(message || "").trim();
  if (!trimmed) {
    throw new Error("message cannot be empty");
  }

  const messages = [
    {
      role: "user",
      content: trimmed,
    },
  ];

  return postLettaBotCompletion({ lettabotBase, lettabotKey, agentId, messages });
}

function parseCli(argv) {
  const ctx = {
    apiBase: process.env.LCITY_API_BASE || "http://localhost:3001",
    agentIdFile: path.join(".lcity", "agent_id"),
    daemonDir: path.join(process.cwd(), ".lcity"),
    daemonPort: Number(process.env.LCITY_DAEMON_PORT || 48483),
    simKey: process.env.SIM_API_KEY || "",
  };

  const tokens = [...argv];
  while (tokens.length > 0 && tokens[0].startsWith("--")) {
    const token = tokens.shift();
    if (token === "--api-base") ctx.apiBase = tokens.shift() || ctx.apiBase;
    if (token === "--agent-id-file") ctx.agentIdFile = tokens.shift() || ctx.agentIdFile;
    if (token === "--daemon-dir") ctx.daemonDir = tokens.shift() || ctx.daemonDir;
    if (token === "--daemon-port") ctx.daemonPort = Number(tokens.shift() || ctx.daemonPort);
    if (token === "--sim-key") ctx.simKey = tokens.shift() || ctx.simKey;
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

function resolveSimKey(simKey) {
  if (simKey && String(simKey).trim()) return String(simKey).trim();
  throw new Error("missing SIM_API_KEY (set env or pass --sim-key)");
}

async function readRequestBody(req, limit = 4096) {
  const chunks = [];
  let total = 0;
  return new Promise((resolve, reject) => {
    req.on("data", (chunk) => {
      total += chunk.length;
      if (total > limit) {
        req.destroy();
        reject(new Error("body too large"));
        return;
      }
      chunks.push(chunk);
    });
    req.on("end", () => {
      resolve(Buffer.concat(chunks).toString("utf8"));
    });
    req.on("error", reject);
  });
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

function printNotification(notification) {
  if (!notification || typeof notification !== "object") return;
  const mode = notification.mode || notification.type || "unknown";
  const eta = notification.eta_seconds ? ` (eta ~${notification.eta_seconds}s)` : "";
  const message = notification.message || "";
  const line = `[notify:${mode}] ${message}${eta}`.trim();
  if (!line) return;
  console.error(line);
}

async function callApi(ctx, route, { method = "GET", body, requiresAgent = false, requireSimKey = true } = {}) {
  const headers = {};
  if (requireSimKey) {
    headers["x-sim-key"] = resolveSimKey(ctx.simKey);
  }
  if (requiresAgent) {
    headers["x-agent-id"] = resolveAgentId(ctx.agentIdFile);
  }

  const { statusCode, data } = await requestJson(`${ctx.apiBase.replace(/\/$/, "")}${route}`, {
    method,
    headers,
    body,
  });

  let payload = data;
  let notification = null;

  if (data && typeof data === "object") {
    if (Object.prototype.hasOwnProperty.call(data, "data")) {
      payload = data.data;
    }
    if (Object.prototype.hasOwnProperty.call(data, "notification")) {
      notification = data.notification;
    }
  }

  if (notification) {
    printNotification(notification);
  }

  const output = { ok: okStatus(statusCode), status_code: statusCode, data: payload };
  if (notification) output.notification = notification;

  console.log(JSON.stringify(output));
  return output.ok ? 0 : 1;
}

async function notifyDaemon(ctx, { message, agentId }) {
  const trimmedMessage = String(message || "").trim();
  if (!trimmedMessage) {
    throw new Error("message cannot be empty");
  }

  const resolvedAgentId = agentId && String(agentId).trim()
    ? String(agentId).trim()
    : resolveAgentId(ctx.agentIdFile);

  const url = `http://127.0.0.1:${ctx.daemonPort}/notify`;
  const payload = {
    message: trimmedMessage,
    agent_id: resolvedAgentId,
  };

  try {
    const response = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
    const text = await response.text();
    let data = {};
    if (text) {
      try {
        data = JSON.parse(text);
      } catch {
        data = { raw: text };
      }
    }
    const output = { ok: response.ok && data.ok !== false, status_code: response.status, data };
    console.log(JSON.stringify(output));
    return output.ok ? 0 : 1;
  } catch (err) {
    console.log(JSON.stringify({
      ok: false,
      error: `failed to reach daemon notify endpoint: ${err.message}`,
    }));
    return 1;
  }
}

const COMMANDS = {
  health_check: {
    route: "/agents/health",
    requiresAgent: true,
  },
  move_to: {
    route: "/agents/move",
    method: "PATCH",
    requiresAgent: true,
    buildBody: (options) => ({
      location_id: required(options, "location-id"),
    }),
  },
  move_to_agent: {
    handler: handleMoveToAgent,
  },
  list_locations: {
    route: "/locations",
  },
  get_location: {
    route: (_ctx, options) =>
      `/locations/${encodeURIComponent(required(options, "id"))}`,
  },
  nearby_locations: {
    route: (_ctx, options) =>
      `/locations/${encodeURIComponent(required(options, "id"))}/nearby`,
  },
  pathfind: {
    route: (_ctx, options) => {
      const from = encodeURIComponent(required(options, "from"));
      const to = encodeURIComponent(required(options, "to"));
      return `/pathfind?from=${from}&to=${to}`;
    },
  },
  world_time: {
    route: "/world/time",
  },
  list_inventory: {
    route: (ctx) =>
      `/inventory/${encodeURIComponent(resolveAgentId(ctx.agentIdFile))}`,
  },
  board_read: {
    route: "/board",
  },
  board_posts: {
    route: "/board/posts",
  },
  board_post: {
    route: "/board/posts",
    method: "PATCH",
    requiresAgent: true,
    buildBody: (options) => ({
      text: required(options, "text"),
    }),
  },
  board_delete: {
    route: (_ctx, options) =>
      `/board/posts/${encodeURIComponent(required(options, "post-id"))}`,
    method: "DELETE",
    requiresAgent: true,
  },
  board_clear: {
    route: "/board/clear",
    method: "DELETE",
    requiresAgent: true,
  },
};

async function executeDeclarativeCommand(ctx, def, options) {
  if (typeof def.handler === "function") {
    return def.handler(ctx, options);
  }

  const routeBuilder = def.route;
  if (!routeBuilder) {
    throw new Error("command route is not configured");
  }

  const route = typeof routeBuilder === "function" ? routeBuilder(ctx, options) : routeBuilder;
  if (!route) {
    throw new Error("command route resolved to empty value");
  }

  const body = def.buildBody ? def.buildBody(options, ctx) : undefined;
  const method = def.method || "GET";
  const requiresAgent = Boolean(def.requiresAgent);
  const requireSimKey = def.requireSimKey !== false;

  return callApi(ctx, route, {
    method,
    requiresAgent,
    requireSimKey,
    body,
  });
}

async function handleMoveToAgent(ctx, options) {
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

function usage() {
  return [
    "Set SIM_API_KEY env (or use --sim-key) before invoking commands",
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
    "lcity lettabot_notify --message \"Wrap up\"",
    "lcity daemon --start",
    "lcity daemon --stop",
    "lcity daemon --status",
  ];
}

function ensureDir(dir) {
  fs.mkdirSync(dir, { recursive: true });
}

function daemonPidPath(ctx) {
  return path.join(ctx.daemonDir, "daemon.pid");
}

function daemonLogPath(ctx) {
  return path.join(ctx.daemonDir, "daemon.log");
}

function sendJson(res, statusCode, payload) {
  res.writeHead(statusCode, { "Content-Type": "application/json" });
  res.end(JSON.stringify(payload));
}

function wsUrlFromApiBase(apiBase) {
  const trimmed = apiBase.replace(/\/$/, "");
  if (trimmed.startsWith("https://")) return `wss://${trimmed.slice("https://".length)}/ws/events`;
  if (trimmed.startsWith("http://")) return `ws://${trimmed.slice("http://".length)}/ws/events`;
  // fallback
  return `${trimmed}/ws/events`;
}

async function fetchWithTimeout(url, { method = "GET", headers = {}, body, timeoutMs = 750 } = {}) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const resp = await fetch(url, {
      method,
      headers,
      body,
      signal: controller.signal,
    });
    const text = await resp.text();
    let data = {};
    if (text) {
      try {
        data = JSON.parse(text);
      } catch {
        data = { raw: text };
      }
    }
    return { ok: resp.ok, statusCode: resp.status, data };
  } catch (err) {
    return { ok: false, statusCode: 0, data: { error: err.message } };
  } finally {
    clearTimeout(timer);
  }
}

async function daemonStatus(ctx) {
  const url = `http://127.0.0.1:${ctx.daemonPort}/health`;
  const resp = await fetchWithTimeout(url);
  return resp.ok;
}

async function daemonStop(ctx) {
  const running = await daemonStatus(ctx);
  if (!running) {
    console.log(JSON.stringify({ ok: true, already_stopped: true }));
    return 0;
  }

  await fetchWithTimeout(`http://127.0.0.1:${ctx.daemonPort}/shutdown`, {
    method: "POST",
    timeoutMs: 1500,
  });

  // wait a moment for shutdown
  await new Promise((r) => setTimeout(r, 300));
  if (!(await daemonStatus(ctx))) {
    console.log(JSON.stringify({ ok: true, stopped: true }));
    return 0;
  }

  // fallback: kill by pid
  try {
    const pidFile = daemonPidPath(ctx);
    if (fs.existsSync(pidFile)) {
      const pid = Number(fs.readFileSync(pidFile, "utf8").trim());
      if (Number.isFinite(pid)) {
        process.kill(pid);
      }
    }
  } catch (err) {
    console.log(JSON.stringify({ ok: false, error: `failed to stop daemon: ${err.message}` }));
    return 1;
  }

  console.log(JSON.stringify({ ok: true, stopped: true, forced: true }));
  return 0;
}

async function daemonStart(ctx, options) {
  if (await daemonStatus(ctx)) {
    console.log(JSON.stringify({ ok: true, already_running: true }));
    return 0;
  }

  ensureDir(ctx.daemonDir);

  const node = process.execPath;
  // Important on Windows: use fileURLToPath to avoid leading / and %XX escaping.
  const entry = fileURLToPath(new URL("../bin/lcity.mjs", import.meta.url));

  const childArgs = [
    "--api-base",
    ctx.apiBase,
    "--daemon-dir",
    ctx.daemonDir,
    "--daemon-port",
    String(ctx.daemonPort),
    "daemon",
    "--run",
  ];

  // pass through optional overrides
  if (options["ws-url"]) childArgs.push("--ws-url", String(options["ws-url"]));
  if (options["sim-key"]) childArgs.push("--sim-key", String(options["sim-key"]));
  if (options["lettabot-key"]) childArgs.push("--lettabot-key", String(options["lettabot-key"]));
  if (options["lettabot-base"]) childArgs.push("--lettabot-base", String(options["lettabot-base"]));

  const logFile = fs.openSync(daemonLogPath(ctx), "a");
  const spawnOpts = {
    detached: true,
    stdio: ["ignore", logFile, logFile],
    windowsHide: true,
    cwd: process.cwd(),
    env: process.env,
  };
  if (process.platform === "win32") {
    // mimic CREATE_NO_WINDOW behavior in sturdy Windows daemons
    spawnOpts.CREATE_NO_WINDOW = 0x08000000;
  }

  const child = spawn(node, [entry, ...childArgs], spawnOpts);
  child.unref();

  console.log(JSON.stringify({ ok: true, started: true }));
  return 0;
}

function appendLog(ctx, line) {
  try {
    ensureDir(ctx.daemonDir);
    fs.appendFileSync(daemonLogPath(ctx), `${new Date().toISOString()} ${line}${os.EOL}`);
  } catch {
    // ignore logging errors in daemon
  }
}

function writePid(ctx) {
  ensureDir(ctx.daemonDir);
  fs.writeFileSync(daemonPidPath(ctx), String(process.pid), "utf8");
}

function removePid(ctx) {
  try {
    fs.unlinkSync(daemonPidPath(ctx));
  } catch {
    // ignore
  }
}

async function daemonRun(ctx, options) {
  const wsUrl = String(options["ws-url"] || wsUrlFromApiBase(ctx.apiBase));
  const simKey = String(options["sim-key"] || ctx.simKey || process.env.SIM_API_KEY || "");
  const lettabotKey = String(options["lettabot-key"] || process.env.LETTABOT_API_KEY || "");
  const lettabotBase = String(
    options["lettabot-base"] || process.env.LETTABOT_BASE || "https://api.letta.com",
  );

  if (!simKey) {
    console.log(JSON.stringify({ ok: false, error: "missing SIM_API_KEY (or pass --sim-key)" }));
    return 1;
  }
  if (!lettabotKey) {
    console.log(JSON.stringify({ ok: false, error: "missing LETTABOT_API_KEY (or pass --lettabot-key)" }));
    return 1;
  }

  ensureDir(ctx.daemonDir);
  writePid(ctx);
  appendLog(ctx, `daemon starting port=${ctx.daemonPort} ws=${wsUrl}`);

  let shuttingDown = false;

  const server = http.createServer(async (req, res) => {
    if (req.method === "GET" && req.url === "/health") {
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ ok: true }));
      return;
    }
    if (req.method === "POST" && req.url === "/shutdown") {
      shuttingDown = true;
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ ok: true }));
      return;
    }
    if (req.method === "POST" && req.url === "/notify") {
      try {
        const rawBody = await readRequestBody(req, 4096);
        let payload = {};
        try {
          payload = rawBody ? JSON.parse(rawBody) : {};
        } catch {
          sendJson(res, 400, { ok: false, error: "invalid JSON body" });
          return;
        }

        const message = typeof payload.message === "string" ? payload.message.trim() : "";
        if (!message) {
          sendJson(res, 400, { ok: false, error: "missing message" });
          return;
        }

        const agentId = typeof payload.agent_id === "string" ? payload.agent_id.trim() : "";
        if (!agentId) {
          sendJson(res, 400, { ok: false, error: "missing agent_id" });
          return;
        }

        const lettabotResp = await sendLettaBotMessage({
          lettabotBase,
          lettabotKey,
          agentId,
          message,
        });

        if (!lettabotResp.ok) {
          const errorText = await lettabotResp.text();
          sendJson(res, lettabotResp.status || 500, {
            ok: false,
            error: errorText || "LettaBot notify failed",
          });
          return;
        }

        appendLog(ctx, `notify agent=${agentId} len=${message.length} status=${lettabotResp.status}`);
        sendJson(res, 200, { ok: true });
      } catch (err) {
        appendLog(ctx, `notify error=${err.message}`);
        sendJson(res, 500, { ok: false, error: err.message });
      }
      return;
    }
    res.writeHead(404, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ ok: false, error: "not found" }));
  });

  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(ctx.daemonPort, "127.0.0.1", () => resolve());
  });

  // WS loop (reconnect with backoff)
  let backoffMs = 250;
  try {
    while (!shuttingDown) {
      appendLog(ctx, `ws connecting url=${wsUrl}`);
      const ws = new WebSocket(wsUrl, { headers: { "x-sim-key": simKey } });

      let wsOpen = false;
      ws.onopen = () => {
        wsOpen = true;
        backoffMs = 250;
        appendLog(ctx, "ws connected");
      };
      ws.onerror = (err) => {
        appendLog(ctx, `ws error=${err.message || err}`);
      };
      ws.onclose = () => {
        appendLog(ctx, "ws closed");
      };
      ws.onmessage = async (ev) => {
        try {
          const envelope = JSON.parse(String(ev.data || "{}"));
          const targets = Array.isArray(envelope.agent_targets)
            ? envelope.agent_targets
            : [];
          if (targets.length === 0) return;

          for (const agentId of targets) {
            const resp = await wakeAgentViaLettaBot({
              lettabotBase,
              lettabotKey,
              agentId,
              envelope,
            });
            appendLog(
              ctx,
              `wake agent=${agentId} event=${envelope.type || envelope.event_type} status=${resp.status}`,
            );
          }
        } catch (err) {
          appendLog(ctx, `onmessage error=${err.message}`);
        }
      };

      // wait until shutdown or socket closes
      while (!shuttingDown && ws.readyState !== WebSocket.CLOSED) {
        await sleep(250);
      }

      try {
        if (wsOpen) ws.close();
      } catch {
        // ignore
      }

      if (shuttingDown) break;

      // backoff before reconnect
      await sleep(backoffMs);
      backoffMs = Math.min(backoffMs * 2, 5000);
    }
  } catch (err) {
    appendLog(ctx, `daemon loop error=${err.message}`);
  } finally {
    server.close();
    removePid(ctx);
    appendLog(ctx, "daemon stopped");
  }

  return 0;
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
      case "lettabot_notify": {
        const message = required(options, "message");
        const agentOverride = options["agent-id"] ? String(options["agent-id"]).trim() : undefined;
        return notifyDaemon(ctx, { message, agentId: agentOverride });
      }
      case "daemon": {
        if (options.run === "true") return daemonRun(ctx, options);
        if (options.start === "true") return daemonStart(ctx, options);
        if (options.stop === "true") return daemonStop(ctx);
        if (options.status === "true") {
          const running = await daemonStatus(ctx);
          console.log(JSON.stringify({ ok: true, running }));
          return 0;
        }
        console.log(JSON.stringify({ ok: false, error: "missing --start/--stop/--status (or internal --run)" }));
        return 1;
      }
      default:
        console.log(JSON.stringify({ ok: false, error: `unknown command: ${command}` }));
        return 1;
    }
  } catch (err) {
    const errorData = { ok: false, error: err.message };
    if (err.cause) errorData.cause = err.cause;
    console.log(JSON.stringify(errorData));
    return 1;
  }
}
