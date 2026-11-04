#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  artifactSet: "crates/modules/rustok-pages/src/services/page/artifact_set.rs",
  rollback: "crates/modules/rustok-pages/src/services/page/rollback.rs",
  manifestMigration: "crates/modules/rustok-pages/src/migrations/m20260722_000009_create_page_rollback_operations.rs",
  test: "crates/modules/rustok-pages/tests/artifact_repair_rollback_continuity_postgres.rs",
  evidence: "crates/modules/rustok-pages/contracts/evidence/pages-artifact-repair-rollback-continuity-source.json",
  packet: "crates/modules/rustok-pages/docs/artifact-repair-rollback-continuity.md",
  registry: "crates/modules/rustok-page-builder/contracts/page-builder-fba-registry.json",
};

const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const requireOrdered = (source, markers, label) => {
  let previous = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker, previous + 1);
    if (index < 0) {
      failures.push(`${label}: missing or out of order ${marker}`);
      return;
    }
    previous = index;
  }
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
  console.error("[verify-pages-artifact-repair-rollback-continuity] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);
const registry = JSON.parse(sources.registry);
const pages = registry.consumers?.find((consumer) => consumer.module_slug === "pages");

if (evidence.format !== "pages_artifact_repair_rollback_continuity_source_v1") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_artifact_repair_rollback_continuity_source_unvalidated") {
  failures.push("evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}
for (const value of Object.values(evidence.validation ?? {})) {
  if (value !== false) failures.push("evidence validation must remain unexecuted");
}
, marker, "repair rollback packet");
}
, marker, "parity actualization");
}

if (failures.length > 0) {
  console.error("[verify-pages-artifact-repair-rollback-continuity] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

console.log("[verify-pages-artifact-repair-rollback-continuity] PASS");
