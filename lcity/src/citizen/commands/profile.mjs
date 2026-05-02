import { buildBootstrapProfile, deleteProfile, getActiveProfile, hasProfile, listProfiles, readProfile, setActiveProfile, writeProfile } from "../config.mjs";

function printJson(value) {
  console.log(JSON.stringify(value, null, 2));
}

function requiredName(flags) {
  const name = String(flags.name || "").trim();
  if (!name) {
    throw new Error("missing --name");
  }
  return name;
}

export async function runProfileCommand({ subcommand, flags }) {
  const cwd = flags.cwd || process.cwd();

  switch (subcommand || "list") {
    case "list": {
      const active = getActiveProfile(cwd);
      const profiles = listProfiles(cwd).map((name) => ({ name, active: name === active }));
      printJson({ ok: true, active_profile: active || null, profiles });
      return 0;
    }
    case "show": {
      const name = requiredName(flags);
      const profile = readProfile(name, cwd);
      printJson({ ok: true, name, profile });
      return 0;
    }
    case "init": {
      const name = requiredName(flags);
      if (hasProfile(name, cwd)) {
        throw new Error(`profile already exists: ${name}`);
      }
      const { profile } = buildBootstrapProfile({ cwd, name });
      const profilePath = writeProfile(name, profile, cwd);
      if (String(flags.use || "").toLowerCase() === "true" || flags.use === true) {
        setActiveProfile(name, cwd);
      }
      printJson({ ok: true, name, path: profilePath, profile });
      return 0;
    }
    case "use": {
      const name = requiredName(flags);
      if (!hasProfile(name, cwd)) {
        throw new Error(`profile not found: ${name}`);
      }
      setActiveProfile(name, cwd);
      printJson({ ok: true, active_profile: name });
      return 0;
    }
    case "delete": {
      const name = requiredName(flags);
      deleteProfile(name, cwd);
      printJson({ ok: true, deleted: name });
      return 0;
    }
    default:
      throw new Error(`unknown profile subcommand: ${subcommand}`);
  }
}
