#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const adminRoot = path.resolve(scriptDir, "..");
const stagingDir = path.resolve(process.env.TRUNK_STAGING_DIR || path.join(adminRoot, "dist"));
const executable = process.platform === "win32"
  ? path.join(adminRoot, "node_modules", ".bin", "tailwindcss.cmd")
  : path.join(adminRoot, "node_modules", ".bin", "tailwindcss");

if (!fs.existsSync(executable)) {
  console.error(
    `Missing ${path.relative(adminRoot, executable)}. Run npm ci in apps/admin before trunk build.`,
  );
  process.exit(1);
}

fs.mkdirSync(stagingDir, { recursive: true });
const output = path.join(stagingDir, "output.css");
const result = spawnSync(executable, ["-i", "input.css", "-o", output, "--minify"], {
  cwd: adminRoot,
  env: process.env,
  stdio: "inherit",
  shell: false,
});

if (result.error) {
  console.error(`Failed to start Tailwind CLI: ${result.error.message}`);
  process.exit(1);
}
if (result.status !== 0) {
  process.exit(result.status || 1);
}
if (!fs.existsSync(output) || fs.statSync(output).size === 0) {
  console.error(`Tailwind hook did not create a non-empty ${output}`);
  process.exit(1);
}

fs.closeSync(fs.openSync(path.join(stagingDir, ".gitkeep"), "a"));
console.log(`✔ wrote ${output}`);
