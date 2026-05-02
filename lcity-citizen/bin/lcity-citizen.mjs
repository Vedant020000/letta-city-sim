#!/usr/bin/env node
import { run } from "../../lcity/src/index.mjs";

const code = await run(["citizen", ...process.argv.slice(2)]);
process.exit(code);
