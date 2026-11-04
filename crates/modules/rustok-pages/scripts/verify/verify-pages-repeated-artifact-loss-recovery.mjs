#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  activation: "crates/modules/rustok-pages/src/services/page/artifact_binding_replacement.rs",
  artifactSet: "crates/modules/rustok-pages/src/services/page/artifact_set.rs",
  test: "crates/modules/rustok-pages/tests/artifact_repeated_loss_recovery_postgres.rs",
  evidence: "crates/modules/rustok-pages/contracts/evidence/pages-repeated-artifact-loss-recovery-source.json",
  plan: "crates/modules/rustok-pages/docs/implementation-plan.md",
  fba: "crates/modules/rustok-page-builder/contracts/page-builder-fba-registry.json",
};

const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};

for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(absolute(relativePath))) {
    failures.push(`${label}: missing ${relativePath}`);
    continue;
  }
  const stats = fs.lstatSync(absolute(relativePath));
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${label}: ${relativePath} must be a regular non-symlink file`);
  }
}
if (failures.length > 0) {
  console.error("[verify-pages-repeated-artifact-loss-recovery] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);
if (evidence.format !== "pages_repeated_artifact_loss_recovery_source_v1") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_repeated_artifact_loss_recovery_source_unvalidated") {
  failures.push("evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`evidence validation.${key} must remain false`);
}
, marker, "repeated-loss overlay");
}
for (const marker of [
  "repeated artifact-loss recovery",
  "latest repair state per locale",
  "artifact_repeated_loss_recovery_postgres.rs",
  "verify-pages-repeated-artifact-loss-recovery.mjs",
]) {
  need(sources.plan, marker, "Pages implementation plan");
}
for (const marker of [
  "repeated_locale_recovery_supported",
  "latest_repair_state_per_locale",
  "pages_repeated_artifact_loss_recovery_verifier",
]) {
  need(sources.fba, marker, "Page Builder FBA registry");
}
for (const marker of [
  "automatic_audit_to_repair",
  "compile_materialized_static_landing",
  "sanitize_static_landing_project",
]) {
  forbid(sources.activation, marker, "activation owner boundary");
}

if (failures.length > 0) {
  console.error("[verify-pages-repeated-artifact-loss-recovery] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}
console.log("[verify-pages-repeated-artifact-loss-recovery] PASS");
