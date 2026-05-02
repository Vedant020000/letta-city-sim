import { run as runAdmin } from "./admin/index.mjs";
import { run as runCitizen } from "./citizen/index.mjs";

export async function run(argv) {
  const tokens = Array.isArray(argv) ? [...argv] : [];
  const command = tokens[0] || "";

  if (command === "citizen") {
    return runCitizen(tokens.slice(1));
  }

  return runAdmin(tokens);
}
