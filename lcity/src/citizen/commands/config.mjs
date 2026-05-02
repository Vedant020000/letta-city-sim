import { flattenResolvedConfig, resolveRuntimeConfig, validateResolvedConfig } from "../config.mjs";

function printJson(value) {
  console.log(JSON.stringify(value, null, 2));
}

export async function runConfigCommand({ subcommand, flags }) {
  const resolved = resolveRuntimeConfig({ flags, cwd: flags.cwd || process.cwd() });

  switch (subcommand || "show") {
    case "show": {
      printJson({ ok: true, config: flattenResolvedConfig(resolved) });
      return 0;
    }
    case "validate": {
      const validation = validateResolvedConfig(resolved);
      printJson({ ok: validation.ok, validation, config: flattenResolvedConfig(resolved) });
      return validation.ok ? 0 : 1;
    }
    case "sources": {
      const config = flattenResolvedConfig(resolved);
      printJson({ ok: true, sources: config });
      return 0;
    }
    default:
      throw new Error(`unknown config subcommand: ${subcommand}`);
  }
}
