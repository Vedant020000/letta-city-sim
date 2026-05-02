#!/usr/bin/env node
import { run } from "../src/citizen/index.mjs";

const code = await run(process.argv.slice(2));
process.exit(code);
