#!/usr/bin/env node

import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const comparator = path.join(scriptDir, "verify-api-compatibility.mjs");
const result = spawnSync(process.execPath, [comparator, "--self-test"], {
  stdio: "inherit",
});

if (result.error) {
  console.error(`API compatibility comparator self-test failed: ${result.error.message}`);
  process.exit(1);
}

process.exit(result.status ?? 1);
