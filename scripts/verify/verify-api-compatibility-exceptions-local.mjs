#!/usr/bin/env node

import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(scriptDir, "../..");
const validator = path.join(scriptDir, "verify-api-compatibility-exceptions.mjs");
const register = path.join(repositoryRoot, "docs/api/compatibility-exceptions.json");
const result = spawnSync(process.execPath, [validator, "--file", register], {
  stdio: "inherit",
});

if (result.error) {
  console.error(`API compatibility exception verification failed: ${result.error.message}`);
  process.exit(1);
}

process.exit(result.status ?? 1);
