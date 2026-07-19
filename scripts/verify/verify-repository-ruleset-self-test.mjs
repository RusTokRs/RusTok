#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const directory = path.dirname(fileURLToPath(import.meta.url));
const commands = [
  [path.join(directory, "verify-repository-ruleset-contract.mjs"), "--self-test"],
  [path.join(directory, "verify-repository-ruleset-admin-payload.mjs")],
  [path.join(directory, "verify-repository-ruleset-structure.mjs")],
  [path.join(directory, "verify-main-protection-rollout.mjs")],
];

for (const command of commands) {
  const result = spawnSync(process.execPath, command, { stdio: "inherit" });
  if (result.error) {
    console.error(result.error.message);
    process.exit(1);
  }
  if ((result.status ?? 1) !== 0) process.exit(result.status ?? 1);
}
