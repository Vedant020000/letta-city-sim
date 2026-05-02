import { runRunCommand } from "./run.mjs";

export async function runInteractiveCommand({ flags }) {
  return runRunCommand({
    flags: {
      ...flags,
      mode: "interactive",
      tui: flags.tui !== false,
    },
  });
}
