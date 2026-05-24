#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const checks = [
  "verify-page-builder-contract-parity.mjs",
  "verify-page-builder-fallback-profiles.mjs",
  "verify-page-builder-toggle-profiles-consistency.mjs",
  "verify-page-builder-terminology.mjs",
];

for (const check of checks) {
  const checkPath = path.join(__dirname, check);
  console.log(`[verify-page-builder-fba-baseline] running ${check}`);
  const run = spawnSync(process.execPath, [checkPath], { stdio: "inherit" });
  if (run.status !== 0) {
    console.error("[verify-page-builder-fba-baseline] FAIL");
    process.exit(run.status ?? 1);
  }
}

console.log("[verify-page-builder-fba-baseline] PASS");
