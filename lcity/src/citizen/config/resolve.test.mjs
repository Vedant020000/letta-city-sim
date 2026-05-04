import assert from "node:assert/strict";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import { resolveRuntimeConfig } from "../config.mjs";

function withEnv(vars, fn) {
  const previous = new Map();
  for (const key of Object.keys(vars)) {
    previous.set(key, process.env[key]);
    if (vars[key] == null) {
      delete process.env[key];
    } else {
      process.env[key] = vars[key];
    }
  }

  try {
    return fn();
  } finally {
    for (const [key, value] of previous.entries()) {
      if (value == null) {
        delete process.env[key];
      } else {
        process.env[key] = value;
      }
    }
  }
}

function tempCwd() {
  return mkdtempSync(path.join(tmpdir(), "lcity-citizen-config-"));
}

test("derives Railway bundled citizen websocket URL from /api base", () => {
  const cwd = tempCwd();
  try {
    const resolved = withEnv({
      LCITY_API_BASE: null,
      LCITY_CITIZEN_WS_URL: null,
    }, () => resolveRuntimeConfig({
      cwd,
      flags: {
        apiBase: "https://hosted-world.example/api",
      },
    }));

    assert.equal(
      resolved.world.ws_url.value,
      "wss://hosted-world.example/ws/citizen",
    );
  } finally {
    rmSync(cwd, { recursive: true, force: true });
  }
});

test("supports explicit citizen websocket URL override", () => {
  const cwd = tempCwd();
  try {
    const resolved = withEnv({
      LCITY_API_BASE: null,
      LCITY_CITIZEN_WS_URL: "wss://example.test/custom/citizen",
    }, () => resolveRuntimeConfig({
      cwd,
      flags: {
        apiBase: "https://hosted-world.example/api",
      },
    }));

    assert.equal(resolved.world.ws_url.value, "wss://example.test/custom/citizen");
    assert.equal(resolved.world.ws_url.source, "env:LCITY_CITIZEN_WS_URL");
  } finally {
    rmSync(cwd, { recursive: true, force: true });
  }
});
