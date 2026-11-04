#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const files = {
  builder: "crates/modules/rustok-pages/admin/src/builder.rs",
  adapter: "crates/modules/rustok-pages/admin/src/builder_rollout_settings.rs",
  composition: "crates/modules/rustok-pages/admin/src/composition.rs",
  adminMain: "apps/admin/src/main.rs",
  evidence: "crates/modules/rustok-pages/contracts/evidence/pages-builder-rollout-binding-source.json",
  healthBinding: "crates/modules/rustok-pages/contracts/evidence/pages-builder-provider-health-consumer-binding-source.json",
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
  console.error("[verify-pages-builder-rollout-binding] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const sources = Object.fromEntries(Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]));
const evidence = JSON.parse(sources.evidence);
const healthBinding = JSON.parse(sources.healthBinding);
const gate = JSON.parse(sources.gate);

, marker, "rollout actualization");
, marker, "current parity actualization");

if (failures.length) {
  console.error("[verify-pages-builder-rollout-binding] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-builder-rollout-binding] PASS bindings=ui+ssr+browser_intent rollout=server_owned health=source_ready runtime_evidence=pending");
