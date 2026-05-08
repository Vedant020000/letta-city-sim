import { runMapCommand } from "./commands/map.mjs";

const SUBCOMMANDS = {
  map: runMapCommand,
};

export async function run(argv) {
  const tokens = Array.isArray(argv) ? [...argv] : [];
  const sub = tokens[0] || "";

  if (sub === "map") {
    return runMapCommand({ flags: parseFlags(tokens.slice(1)) });
  }

  console.log(JSON.stringify({
    ok: false,
    error: `Unknown town subcommand: ${sub}`,
    usage: [
      "lcity town map [--sim-key <key>] [--api-base <url>] [--poll-ms <ms>]",
    ],
  }, null, 2));
  return 1;
}

function parseFlags(tokens) {
  const flags = {};
  let i = 0;
  while (i < tokens.length) {
    const token = tokens[i];
    if (token.startsWith("--")) {
      const key = token.slice(2).replace(/-/g, "_");
      const next = tokens[i + 1];
      if (next && !next.startsWith("--")) {
        flags[key] = next;
        i += 2;
      } else {
        flags[key] = true;
        i += 1;
      }
    } else {
      i += 1;
    }
  }
  return flags;
}
