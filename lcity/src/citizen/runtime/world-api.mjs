function normalizeApiBase(apiBase) {
  return String(apiBase || "").trim().replace(/\/$/, "");
}

async function requestJson(url, { method = "GET", headers = {}, body } = {}) {
  try {
    const response = await fetch(url, {
      method,
      headers: {
        ...headers,
        ...(body !== undefined ? { "Content-Type": "application/json" } : {}),
      },
      ...(body !== undefined ? { body: JSON.stringify(body) } : {}),
    });

    const text = await response.text();
    let payload = null;
    if (text) {
      try {
        payload = JSON.parse(text);
      } catch {
        payload = { raw: text };
      }
    }

    return {
      ok: response.ok,
      status: response.status,
      payload,
    };
  } catch (error) {
    return {
      ok: false,
      status: 0,
      payload: {
        error: {
          code: "network_error",
          message: error instanceof Error ? error.message : String(error),
        },
      },
    };
  }
}

function buildWorldToolHeaders(config) {
  const agentId = config.world.auth.city_agent_id.value;
  const mode = config.world.tool_auth.mode.value;

  if (mode === "sim_key") {
    return {
      "x-sim-key": config.world.tool_auth.sim_api_key.value,
      "x-agent-id": agentId,
    };
  }

  if (mode === "bearer_token") {
    return {
      Authorization: `Bearer ${config.world.auth.bearer_token.value}`,
      "x-agent-id": agentId,
    };
  }

  throw new Error(`unsupported world.tool_auth.mode: ${mode}`);
}

function normalizeApiResponse(response) {
  const body = response.payload;
  if (!body || typeof body !== "object") {
    return {
      ok: response.ok,
      status_code: response.status,
      data: body,
    };
  }

  if (Object.prototype.hasOwnProperty.call(body, "data")) {
    return {
      ok: response.ok,
      status_code: response.status,
      data: body.data,
      ...(Object.prototype.hasOwnProperty.call(body, "notification") ? { notification: body.notification } : {}),
    };
  }

  return {
    ok: response.ok,
    status_code: response.status,
    data: body,
  };
}

export async function fetchToolManifest(config) {
  const apiBase = normalizeApiBase(config.world.api_base.value);
  const agentId = config.world.auth.city_agent_id.value;
  const response = await requestJson(`${apiBase}/agents/${encodeURIComponent(agentId)}/tool-manifest`, {
    method: "GET",
    headers: {
      "x-agent-id": agentId,
    },
  });

  const normalized = normalizeApiResponse(response);
  if (!response.ok || !normalized.data) {
    throw new Error(
      normalized.data?.error?.message
      || response.payload?.error?.message
      || `failed to fetch tool manifest (HTTP ${response.status})`,
    );
  }

  return normalized.data;
}

export async function invokeWorldTool(config, toolDefinition, args) {
  const apiBase = normalizeApiBase(config.world.api_base.value);
  const response = await requestJson(`${apiBase}${toolDefinition.endpoint}`, {
    method: toolDefinition.method || "POST",
    headers: buildWorldToolHeaders(config),
    body: args,
  });

  return normalizeApiResponse(response);
}

export async function invokeCitizenLifecycle(config, wake, action, args = {}) {
  const apiBase = normalizeApiBase(config.world.api_base.value);
  const response = await requestJson(`${apiBase}/v1/citizen/action`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${config.world.auth.bearer_token.value}`,
      "x-agent-id": config.world.auth.city_agent_id.value,
      "x-wake-token": wake.wake_token,
    },
    body: {
      action,
      args,
      client_event_id: `ce_${globalThis.crypto.randomUUID()}`,
      wake_event_id: wake.event_id,
    },
  });

  return {
    ok: response.ok,
    status_code: response.status,
    body: response.payload,
  };
}
