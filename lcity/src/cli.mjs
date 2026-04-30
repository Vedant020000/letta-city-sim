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

function normalizeInterrupt(interrupt) {
  if (!interrupt || typeof interrupt !== "object") {
    throw new Error("interrupt must be an object");
  }

  const agentId = String(interrupt.agentId || "").trim();
  if (!agentId) {
    throw new Error("interrupt missing agentId");
  }

  const kind = String(interrupt.kind || "world_event").trim();
  const cause = String(interrupt.cause || kind).trim();
  const source = String(interrupt.source || "system").trim();
  const transport = String(interrupt.transport || "lettabot_completion").trim();
  const message = interrupt.message == null ? null : String(interrupt.message).trim();

  if (kind === "manual_message" && !message) {
    throw new Error("manual_message interrupt requires message");
  }

  return {
    ...interrupt,
    agentId,
    kind,
    cause,
    source,
    transport,
    message,
    payload: interrupt.payload && typeof interrupt.payload === "object" ? interrupt.payload : null,
  };
}

function createWorldEventInterrupt({ agentId, envelope, transport = "lettabot_completion" }) {
  const eventType = typeof envelope?.type === "string" && envelope.type.trim()
    ? envelope.type.trim()
    : typeof envelope?.event_type === "string" && envelope.event_type.trim()
      ? envelope.event_type.trim()
      : "world_event";

  return normalizeInterrupt({
    agentId,
    kind: "world_event",
    cause: eventType,
    source: "world_api",
    payload: envelope,
    transport,
  });
}

function createManualInterrupt({ agentId, message, transport = "lettabot_completion" }) {
  return normalizeInterrupt({
    agentId,
    kind: "manual_message",
    cause: "manual_message",
    source: "cli",
    message,
    transport,
  });
}

async function interruptViaLettaBotCompletion({ lettabotBase, lettabotKey, interrupt }) {
  // NOTE: Vedant confirmed LettaBot /v1/chat/completions is a persistent agent call.
  // All wake/interrupt mechanisms normalize into one interrupt object before transport.
  const messages = interrupt.kind === "manual_message"
    ? [
      {
        role: "user",
        content: interrupt.message,
      },
    ]
    : [
      {
        role: "user",
        content: JSON.stringify({
          kind: interrupt.kind,
          cause: interrupt.cause,
          source: interrupt.source,
          event: interrupt.payload,
        }),
      },
    ];

  return postLettaBotCompletion({
    lettabotBase,
    lettabotKey,
    agentId: interrupt.agentId,
    messages,
  });
}

const INTERRUPT_TRANSPORTS = {
  lettabot_completion: interruptViaLettaBotCompletion,
  sdk: async () => {
    throw new Error("interrupt transport 'sdk' is not implemented yet");
  },
  webhook: async () => {
    throw new Error("interrupt transport 'webhook' is not implemented yet");
  },
};

async function interruptAgent({ lettabotBase, lettabotKey, interrupt }) {
  const normalized = normalizeInterrupt(interrupt);
  const adapter = INTERRUPT_TRANSPORTS[normalized.transport];
  if (!adapter) {
    throw new Error(`unknown interrupt transport: ${normalized.transport}`);
  }

  const response = await adapter({
    lettabotBase,
    lettabotKey,
    interrupt: normalized,
  });

  return {
    response,
    interrupt: normalized,
    transport: normalized.transport,
  };
}

function parseCli(argv) {
  const defaultApiBase = process.env.LCITY_API_BASE
    || readOptionalFile(path.join(".lcity", "api_base"))
    || "http://localhost:3001";
  const ctx = {
    apiBase: defaultApiBase,
    agentIdFile: path.join(".lcity", "agent_id"),
    agentToken: process.env.LCITY_AGENT_TOKEN || "",
    agentTokenFile: process.env.LCITY_AGENT_TOKEN_FILE || path.join(".lcity", "agent_token"),
    apiBaseFile: path.join(".lcity", "api_base"),
    daemonDir: path.join(process.cwd(), ".lcity"),
    daemonPort: Number(process.env.LCITY_DAEMON_PORT || 48483),
    simKey: process.env.SIM_API_KEY || "",
  };

  const tokens = [...argv];
  while (tokens.length > 0 && tokens[0].startsWith("--")) {
    const token = tokens.shift();
    if (token === "--api-base") ctx.apiBase = tokens.shift() || ctx.apiBase;
    if (token === "--agent-id-file") ctx.agentIdFile = tokens.shift() || ctx.agentIdFile;
    if (token === "--agent-token") ctx.agentToken = tokens.shift() || ctx.agentToken;
    if (token === "--agent-token-file") ctx.agentTokenFile = tokens.shift() || ctx.agentTokenFile;
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

function readOptionalFile(filePath) {
  try {
    if (!fs.existsSync(filePath)) return "";
    return fs.readFileSync(filePath, "utf8").trim();
  } catch {
    return "";
  }
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

function resolveTargetAgentId(ctx, options) {
  const override = options["agent-id"];
  if (override && String(override).trim()) {
    return String(override).trim();
  }

  return resolveAgentId(ctx.agentIdFile);
}

function buildJobAssignmentBody(options) {
  const body = {};

  if (Object.prototype.hasOwnProperty.call(options, "primary")) {
    body.is_primary = String(options.primary).trim().toLowerCase() === "true";
  }

  if (Object.prototype.hasOwnProperty.call(options, "notes")) {
    body.notes = String(options.notes);
  }

  return body;
}

function resolveSimKey(simKey) {
  if (simKey && String(simKey).trim()) return String(simKey).trim();
  throw new Error("missing SIM_API_KEY (set env or pass --sim-key)");
}

function resolveAgentToken(ctx, { required = false } = {}) {
  const token = ctx.agentToken && String(ctx.agentToken).trim()
    ? String(ctx.agentToken).trim()
    : readOptionalFile(ctx.agentTokenFile);

  if (token) return token;
  if (required) {
    throw new Error("missing LCITY_AGENT_TOKEN (set env, pass --agent-token, or run register_token)");
  }
  return "";
}

function buildAuthHeaders(ctx, { requiresAgent = false, requireSimKey = true, useAgentToken = true } = {}) {
  const headers = {};
  const token = useAgentToken ? resolveAgentToken(ctx) : "";

  if (token) {
    headers.Authorization = `Bearer ${token}`;
    return headers;
  }

  if (requireSimKey) {
    headers["x-sim-key"] = resolveSimKey(ctx.simKey);
  }
  if (requiresAgent) {
    headers["x-agent-id"] = resolveAgentId(ctx.agentIdFile);
  }

  return headers;
}

function normalizeWorldApiBase(world) {
  const trimmed = String(world || "").trim().replace(/\/$/, "");
  if (!trimmed) throw new Error("missing --world");
  if (trimmed.endsWith("/api")) return trimmed;
  return `${trimmed}/api`;
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

async function callApi(
  ctx,
  route,
  { method = "GET", body, requiresAgent = false, requireSimKey = true, useAgentToken = true } = {},
) {
  const headers = buildAuthHeaders(ctx, { requiresAgent, requireSimKey, useAgentToken });

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
    interrupt: createManualInterrupt({
      agentId: resolvedAgentId,
      message: trimmedMessage,
    }),
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

function buildIntentionBody(options, { status } = {}) {
  const body = {};
  if (options.summary) body.summary = String(options.summary);
  if (options.reason) body.reason = String(options.reason);
  if (options["expected-location-id"]) {
    body.expected_location_id = String(options["expected-location-id"]);
  }
  if (options["expected-action"]) body.expected_action = String(options["expected-action"]);
  if (options.outcome) body.outcome = String(options.outcome);
  if (status) body.status = status;
  return body;
}

function registerToken(ctx, options) {
  const apiBase = normalizeWorldApiBase(required(options, "world"));
  const agentId = required(options, "agent-id");
  const token = required(options, "token");

  ensureDir(path.dirname(ctx.agentIdFile));
  ensureDir(path.dirname(ctx.agentTokenFile));
  ensureDir(path.dirname(ctx.apiBaseFile));
  fs.writeFileSync(ctx.agentIdFile, `${agentId}\n`, "utf8");
  fs.writeFileSync(ctx.agentTokenFile, `${token}\n`, "utf8");
  fs.writeFileSync(ctx.apiBaseFile, `${apiBase}\n`, "utf8");

  console.log(JSON.stringify({
    ok: true,
    data: {
      api_base: apiBase,
      agent_id: agentId,
      agent_id_file: ctx.agentIdFile,
      agent_token_file: ctx.agentTokenFile,
    },
  }));
  return 0;
}

async function currentIntentionId(ctx) {
  const agentId = resolveAgentId(ctx.agentIdFile);
  const response = await requestJson(
    `${ctx.apiBase.replace(/\/$/, "")}/agents/${encodeURIComponent(agentId)}/intentions/current`,
    { headers: buildAuthHeaders(ctx) },
  );
  if (!okStatus(response.statusCode)) {
    throw new Error(`failed to load current intention: ${JSON.stringify(response.data)}`);
  }
  const intention = response.data && Object.prototype.hasOwnProperty.call(response.data, "data")
    ? response.data.data
    : response.data;
  if (!intention || !intention.id) {
    throw new Error("agent has no active intention");
  }
  return intention.id;
}

async function updateIntentionStatus(ctx, options, status) {
  const agentId = resolveAgentId(ctx.agentIdFile);
  const intentionId = options["intention-id"]
    ? String(options["intention-id"]).trim()
    : await currentIntentionId(ctx);

  return callApi(ctx, `/agents/${encodeURIComponent(agentId)}/intentions/${encodeURIComponent(intentionId)}`, {
    method: "PATCH",
    body: buildIntentionBody(options, { status }),
  });
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
    sleep: {
      route: "/agents/sleep",
      method: "POST",
      requiresAgent: true,
    },
    wake_up: {
      route: "/agents/sleep",
      method: "DELETE",
      requiresAgent: true,
    },
    list_inventory: {
      route: (ctx) =>
        `/inventory/${encodeURIComponent(resolveAgentId(ctx.agentIdFile))}`,
  },
  list_jobs: {
    route: "/jobs",
    requireSimKey: false,
  },
  get_job: {
    route: (_ctx, options) =>
      `/jobs/${encodeURIComponent(required(options, "id"))}`,
    requireSimKey: false,
  },
  list_agent_jobs: {
    route: (ctx, options) =>
      `/agents/${encodeURIComponent(resolveTargetAgentId(ctx, options))}/jobs`,
    requireSimKey: false,
  },
  list_job_agents: {
    route: (_ctx, options) =>
      `/jobs/${encodeURIComponent(required(options, "job-id"))}/agents`,
    requireSimKey: false,
  },
  assign_job: {
    route: (ctx, options) =>
      `/agents/${encodeURIComponent(resolveTargetAgentId(ctx, options))}/jobs/${encodeURIComponent(required(options, "job-id"))}`,
    method: "PATCH",
    buildBody: (options) => buildJobAssignmentBody(options),
  },
  remove_job: {
    route: (ctx, options) =>
      `/agents/${encodeURIComponent(resolveTargetAgentId(ctx, options))}/jobs/${encodeURIComponent(required(options, "job-id"))}`,
    method: "DELETE",
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
  use_item: {
    route: "/agents/use-item",
    method: "POST",
    requiresAgent: true,
    buildBody: (options) => ({
      item_id: required(options, "item-id"),
      quantity: parseInt(required(options, "quantity"), 10),
    }),
  },
  economy_update: {
    route: (ctx) => `/agents/${encodeURIComponent(resolveAgentId(ctx.agentIdFile))}/economy`,
    method: "PATCH",
    buildBody: (options) => ({
      amount_cents: parseInt(required(options, "amount-cents"), 10),
      reason: options["reason"],
    }),
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
      "lcity sleep",
      "lcity wake_up",
      "lcity list_inventory",
    "lcity list_jobs",
    "lcity get_job --id dispatcher",
    "lcity list_agent_jobs [--agent-id eddy_lin]",
    "lcity list_job_agents --job-id dispatcher",
    "lcity assign_job --job-id writer [--agent-id eddy_lin] [--primary] [--notes \"Draft docs\"]",
    "lcity remove_job --job-id writer [--agent-id eddy_lin]",
    "lcity board_read",
    "lcity board_posts",
    "lcity board_post --text \"Town hall at 6 PM\"",
    "lcity board_delete --post-id <id>",
    "lcity board_clear",
    "lcity create_agent_token --agent-id eddy_lin [--label \"office hours\"]",
    "lcity list_agent_tokens --agent-id eddy_lin",
    "lcity revoke_agent_token --token-id <id>",
    "lcity register_token --world http://localhost:3000 --agent-id eddy_lin --token lcity_agent_...",
    "lcity whoami",
    "lcity current_intention",
    "lcity list_intentions",
    "lcity set_intention --summary \"Find sheet music\" --reason \"I want something new to practice\"",
    "lcity update_intention --intention-id <id> --summary \"...\"",
    "lcity complete_intention [--intention-id <id>] --outcome \"Found a lead\"",
    "lcity fail_intention [--intention-id <id>] --outcome \"Archive was closed\"",
    "lcity abandon_intention [--intention-id <id>] --outcome \"Changed plans\"",
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

        let interrupt;
        if (payload.interrupt && typeof payload.interrupt === "object") {
          interrupt = normalizeInterrupt(payload.interrupt);
        } else {
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

          interrupt = createManualInterrupt({ agentId, message });
        }

        const result = await interruptAgent({
          lettabotBase,
          lettabotKey,
          interrupt,
        });
        const { response: lettabotResp } = result;

        if (!lettabotResp.ok) {
          const errorText = await lettabotResp.text();
          sendJson(res, lettabotResp.status || 500, {
            ok: false,
            error: errorText || "LettaBot notify failed",
          });
          return;
        }

        appendLog(
          ctx,
          `interrupt agent=${interrupt.agentId} cause=${interrupt.cause} transport=${result.transport} status=${lettabotResp.status}`,
        );
        sendJson(res, 200, {
          ok: true,
          transport: result.transport,
          cause: interrupt.cause,
        });
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
            const result = await interruptAgent({
              lettabotBase,
              lettabotKey,
              interrupt: createWorldEventInterrupt({ agentId, envelope }),
            });
            const { response: resp, interrupt } = result;
            appendLog(
              ctx,
              `interrupt agent=${interrupt.agentId} cause=${interrupt.cause} transport=${result.transport} status=${resp.status}`,
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
      case "list_jobs":
        return callApi(ctx, "/jobs", { requireSimKey: false });
      case "get_job":
        return callApi(ctx, `/jobs/${encodeURIComponent(required(options, "id"))}`, {
          requireSimKey: false,
        });
      case "list_agent_jobs": {
        const agentId = resolveTargetAgentId(ctx, options);
        return callApi(ctx, `/agents/${encodeURIComponent(agentId)}/jobs`, {
          requireSimKey: false,
        });
      }
      case "list_job_agents":
        return callApi(ctx, `/jobs/${encodeURIComponent(required(options, "job-id"))}/agents`, {
          requireSimKey: false,
        });
      case "assign_job": {
        const agentId = resolveTargetAgentId(ctx, options);
        return callApi(ctx, `/agents/${encodeURIComponent(agentId)}/jobs/${encodeURIComponent(required(options, "job-id"))}`, {
          method: "PATCH",
          body: buildJobAssignmentBody(options),
        });
      }
      case "remove_job": {
        const agentId = resolveTargetAgentId(ctx, options);
        return callApi(ctx, `/agents/${encodeURIComponent(agentId)}/jobs/${encodeURIComponent(required(options, "job-id"))}`, {
          method: "DELETE",
        });
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
      case "create_agent_token":
        return callApi(ctx, `/admin/agents/${encodeURIComponent(required(options, "agent-id"))}/tokens`, {
          method: "POST",
          body: { label: options.label ? String(options.label) : undefined },
          useAgentToken: false,
        });
      case "list_agent_tokens":
        return callApi(ctx, `/admin/agents/${encodeURIComponent(required(options, "agent-id"))}/tokens`, {
          useAgentToken: false,
        });
      case "revoke_agent_token":
        return callApi(ctx, `/admin/agent-tokens/${encodeURIComponent(required(options, "token-id"))}`, {
          method: "DELETE",
          useAgentToken: false,
        });
      case "register_token":
        return registerToken(ctx, options);
      case "whoami":
        return callApi(ctx, "/agents/health", { requiresAgent: true });
      case "current_intention": {
        const agentId = resolveAgentId(ctx.agentIdFile);
        return callApi(ctx, `/agents/${encodeURIComponent(agentId)}/intentions/current`);
      }
      case "list_intentions": {
        const agentId = resolveAgentId(ctx.agentIdFile);
        return callApi(ctx, `/agents/${encodeURIComponent(agentId)}/intentions`);
      }
      case "set_intention": {
        const agentId = resolveAgentId(ctx.agentIdFile);
        return callApi(ctx, `/agents/${encodeURIComponent(agentId)}/intentions`, {
          method: "POST",
          body: {
            ...buildIntentionBody(options),
            summary: required(options, "summary"),
            reason: required(options, "reason"),
          },
        });
      }
      case "update_intention": {
        const agentId = resolveAgentId(ctx.agentIdFile);
        const intentionId = required(options, "intention-id");
        return callApi(ctx, `/agents/${encodeURIComponent(agentId)}/intentions/${encodeURIComponent(intentionId)}`, {
          method: "PATCH",
          body: buildIntentionBody(options),
        });
      }
      case "complete_intention":
        required(options, "outcome");
        return updateIntentionStatus(ctx, options, "completed");
      case "fail_intention":
        required(options, "outcome");
        return updateIntentionStatus(ctx, options, "failed");
      case "abandon_intention":
        required(options, "outcome");
        return updateIntentionStatus(ctx, options, "abandoned");
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
