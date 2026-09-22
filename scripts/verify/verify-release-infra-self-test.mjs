#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const result = spawnSync(
  process.execPath,
  ["scripts/verify/verify-release-infrastructure-approval.mjs", "--self-test"],
  {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: "pipe",
  },
);

if (result.error) {
  console.error(`release infrastructure self-test failed to start: ${result.error.message}`);
  process.exit(1);
}
process.stdout.write(result.stdout || "");
process.stderr.write(result.stderr || "");
process.exit(result.status ?? 1);
