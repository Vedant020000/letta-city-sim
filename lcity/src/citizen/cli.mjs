import { parseCli, usage } from "./config.mjs";
import { runConfigCommand } from "./commands/config.mjs";
import { runDoctorCommand } from "./commands/doctor.mjs";
import { runInteractiveCommand } from "./commands/interactive.mjs";
import { runProfileCommand } from "./commands/profile.mjs";
import { runRunCommand } from "./commands/run.mjs";
import { runToolsCommand } from "./commands/tools.mjs";

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
        return runRunCommand({ flags });
      case "interactive":
        return runInteractiveCommand({ flags });
      case "config":
        return runConfigCommand({ subcommand, flags });
      case "profile":
        return runProfileCommand({ subcommand, flags });
      case "doctor":
        return runDoctorCommand({ flags });
      case "tools":
        return runToolsCommand({ subcommand, flags });
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
