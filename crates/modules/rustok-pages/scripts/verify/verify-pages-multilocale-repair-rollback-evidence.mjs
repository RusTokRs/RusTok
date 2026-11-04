#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  service: "crates/modules/rustok-pages/src/services/page/artifact_set.rs",
  rollback: "crates/modules/rustok-pages/src/services/page/rollback.rs",
  activation: "crates/modules/rustok-pages/src/services/page/artifact_binding_replacement.rs",
  test: "crates/modules/rustok-pages/tests/artifact_multilocale_repair_rollback_evidence_postgres.rs",
  repeatedTest: "crates/modules/rustok-pages/tests/artifact_repeated_loss_recovery_postgres.rs",
  evidence: "crates/modules/rustok-pages/contracts/evidence/pages-multilocale-repair-rollback-evidence-source.json",
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
  console.error("[verify-pages-multilocale-repair-rollback-evidence] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);
if (evidence.format !== "pages_multilocale_repair_rollback_evidence_source_v3") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_multilocale_repair_rollback_latest_state_evidence_source_unvalidated") {
  failures.push("evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`evidence validation.${key} must remain false`);
}
, marker, "latest parity overlay");
}
for (const marker of [
  "latest_repair_state_per_locale",
  "repeated_locale_recovery_supported",
  "pages_repeated_artifact_loss_recovery_verifier",
]) {
  need(sources.fba, marker, "Page Builder FBA registry");
}

if (failures.length > 0) {
  console.error("[verify-pages-multilocale-repair-rollback-evidence] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}
console.log("[verify-pages-multilocale-repair-rollback-evidence] PASS");
