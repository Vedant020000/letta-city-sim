import { startTownMap } from "../ui/map.mjs";

function normalizeApiBase(apiBase) {
  return String(apiBase || "http://localhost:8080").trim().replace(/\/$/, "");
}

export async function runMapCommand({ flags }) {
  const simKey = flags.simKey || process.env.SIM_API_KEY || "";
  const apiBase = normalizeApiBase(flags.apiBase || process.env.LCITY_API_BASE || "http://localhost:8080");
  const pollMs = parseInt(flags.pollMs || process.env.LCITY_POLL_MS || "2000", 10);

  if (!simKey) {
    console.log(JSON.stringify({ ok: false, error: "--sim-key or SIM_API_KEY is required" }, null, 2));
    return 1;
  }

  try {
    const ui = await startTownMap({ apiBase, simKey, pollMs });
    return 0;
  } catch (error) {
    console.error(error.message);
    return 1;
  }
}
