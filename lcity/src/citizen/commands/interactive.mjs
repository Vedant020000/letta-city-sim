import { runRunCommand } from "./run.mjs";

// DEPRECATION: `lcity citizen interactive` wraps the legacy wake-driven harness.
// Prefer direct commands: `lcity citizen wait`, `look-around`, `move-to`.
export async function runInteractiveCommand({ flags }) {
  return runRunCommand({
    flags: {
      ...flags,
      mode: "interactive",
      tui: flags.tui !== false,
    },
  });
}
