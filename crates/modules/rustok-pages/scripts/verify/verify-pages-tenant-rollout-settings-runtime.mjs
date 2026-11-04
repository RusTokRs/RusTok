#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const files = {
  runtime: "crates/libs/rustok-api/src/runtime.rs",
  rollout: "crates/modules/rustok-page-builder/src/rollout.rs",
  owner: "crates/modules/rustok-pages/src/graphql/builder_rollout.rs",
  adapter: "crates/modules/rustok-pages/admin/src/builder_rollout_settings.rs",
  builder: "crates/modules/rustok-pages/admin/src/builder.rs",
  composition: "crates/modules/rustok-pages/admin/src/composition.rs",
  adminMain: "apps/admin/src/main.rs",
  evidence: "crates/modules/rustok-pages/contracts/evidence/pages-tenant-rollout-settings-runtime-source.json",
  gate: "crates/modules/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json",
};
const failures = [];
const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
const need = (source, marker, label) => { if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`); };
const forbid = (source, marker, label) => { if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`); };

for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(absolute(relativePath))) { failures.push(`${label}: missing ${relativePath}`); continue; }
  const stats = fs.lstatSync(absolute(relativePath));
  if (!stats.isFile() || stats.isSymbolicLink()) failures.push(`${label}: ${relativePath} must be a regular non-symlink file`);
}
if (failures.length) {
  console.error("[verify-pages-tenant-rollout-settings-runtime] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const sources = Object.fromEntries(Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]));
const evidence = JSON.parse(sources.evidence);
const gate = JSON.parse(sources.gate);

, marker, "actualization");

if (failures.length) {
  console.error("[verify-pages-tenant-rollout-settings-runtime] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-tenant-rollout-settings-runtime] PASS owner=pages_graphql bindings=ui+ssr+browser_intent runtime_matrix_harness=maintainer_execution_pending");
