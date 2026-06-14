#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const checks = [
  "verify-page-builder-contract-parity.mjs",
  "verify-page-builder-contract-registry.mjs",
  "verify-page-builder-consumer-readiness.mjs",
  "verify-page-builder-fallback-profiles.mjs",
  "verify-page-builder-error-catalog-binding.mjs",
  "verify-page-builder-fallback-matrix-docs.mjs",
  "verify-page-builder-wave-evidence-template.mjs",
  "verify-page-builder-wave-evidence-packet.mjs",
  "verify-page-builder-runtime-fallback-gate.mjs",
  "verify-page-builder-pages-fallback-gate.mjs",
  "verify-page-builder-pages-legacy-bridge.mjs",
  "verify-page-builder-next-admin-parity.mjs",
  "verify-page-builder-leptos-admin-parity.mjs",
  "verify-page-builder-flutter-parity.mjs",
  "verify-page-builder-toggle-profiles-consistency.mjs",
  "verify-page-builder-terminology.mjs",
];

const moduleSlug = process.argv[2] ?? "pages";

for (const check of checks) {
  const checkPath = path.join(__dirname, check);
  const args = [checkPath];
  if (
    check === "verify-page-builder-consumer-readiness.mjs" ||
    check === "verify-page-builder-contract-registry.mjs" ||
    check === "verify-page-builder-contract-parity.mjs" ||
    check === "verify-page-builder-fallback-profiles.mjs" ||
    check === "verify-page-builder-error-catalog-binding.mjs" ||
    check === "verify-page-builder-toggle-profiles-consistency.mjs"
  ) {
    args.push(moduleSlug);
  }
  console.log(`[verify-page-builder-fba-baseline] running ${check}`);
  const run = spawnSync(process.execPath, args, { stdio: "inherit" });
  if (run.status !== 0) {
    console.error("[verify-page-builder-fba-baseline] FAIL");
    process.exit(run.status ?? 1);
  }
}

console.log("[verify-page-builder-fba-baseline] PASS");
