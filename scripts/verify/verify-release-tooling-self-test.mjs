#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const commands = [
  ["node", ["scripts/release/verify-release-contract.mjs", "--self-test"]],
  ["node", ["scripts/release/generate-spdx-sbom.mjs", "--self-test"]],
  ["node", ["scripts/release/finalize-release-artifacts.mjs", "--self-test"]],
  ["node", ["scripts/release/extract-release-notes.mjs", "--self-test"]],
  ["bash", ["scripts/release/package-server.sh", "--self-test"]],
];

for (const [command, args] of commands) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: "pipe",
    env: { ...process.env, FORCE_COLOR: "0" },
  });
  if (result.error) {
    console.error(`release tooling self-test failed to start ${command}: ${result.error.message}`);
    process.exit(1);
  }
  if (result.status !== 0) {
    process.stderr.write(result.stdout || "");
    process.stderr.write(result.stderr || "");
    process.exit(result.status || 1);
  }
  process.stdout.write(result.stdout || "");
  process.stderr.write(result.stderr || "");
}

console.log("✔ all release tooling self-tests passed");
