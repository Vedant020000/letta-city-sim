import { parseCli, usage } from "./config.mjs";

export async function run(argv) {
  try {
    const parsed = parseCli(argv);
    const { command, subcommand, flags } = parsed;

    if (flags.help) {
      console.log(JSON.stringify({ ok: true, usage: usage() }, null, 2));
      return 0;
    }

    switch (command) {
      case "run":
        return (await import("./commands/run.mjs")).runRunCommand({ flags });
      case "mock-run":
        return (await import("./commands/mock-run.mjs")).runMockRunCommand({ flags });
      case "interactive":
        return (await import("./commands/interactive.mjs")).runInteractiveCommand({ flags });
      case "config":
        return (await import("./commands/config.mjs")).runConfigCommand({ subcommand, flags });
      case "wait":
      case "look-around":
      case "move-to":
      case "speak-to":
      case "sleep":
      case "wake-up":
      case "check-inventory":
      case "check-world-time":
      case "check-vitals":
      case "check-balance":
      case "set-activity":
        return (await import("./commands/direct.mjs")).runDirectCitizenCommand({ command, flags });
      case "profile":
        return (await import("./commands/profile.mjs")).runProfileCommand({ subcommand, flags });
      case "doctor":
        return (await import("./commands/doctor.mjs")).runDoctorCommand({ flags });
      case "tools":
        return (await import("./commands/tools.mjs")).runToolsCommand({ subcommand, flags });
      case "help":
        console.log(JSON.stringify({ ok: true, usage: usage() }, null, 2));
        return 0;
      default:
        console.log(JSON.stringify({ ok: false, error: `unknown command: ${command}`, usage: usage() }, null, 2));
        return 1;
    }
  } catch (error) {
    console.log(JSON.stringify({ ok: false, error: error.message }, null, 2));
    return 1;
  }
}
