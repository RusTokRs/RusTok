#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  migration: "crates/modules/rustok-pages/src/migrations/m20260807_000015_create_page_artifact_binding_replacements.rs",
  entity: "crates/modules/rustok-pages/src/entities/page_artifact_binding_replacement_operation.rs",
  dto: "crates/modules/rustok-pages/src/dto/artifact_binding_replacement.rs",
  service: "crates/modules/rustok-pages/src/services/page/artifact_binding_replacement.rs",
  rollback: "crates/modules/rustok-pages/src/services/page/rollback.rs",
  artifactService: "crates/modules/rustok-pages/src/services/page_builder_artifact.rs",
  sqliteTest: "crates/modules/rustok-pages/tests/explicit_artifact_binding_replacement_sqlite.rs",
  singleLossTest: "crates/modules/rustok-pages/tests/artifact_loss_activation_recovery_postgres.rs",
  multiLossTest: "crates/modules/rustok-pages/tests/artifact_loss_multilocale_activation_recovery_postgres.rs",
  rollbackLossTest: "crates/modules/rustok-pages/tests/artifact_loss_after_rollback_activation_recovery_postgres.rs",
  repeatedLossTest: "crates/modules/rustok-pages/tests/artifact_repeated_loss_recovery_postgres.rs",
  evidence: "crates/modules/rustok-pages/contracts/evidence/pages-explicit-artifact-binding-replacement-source.json",
  recoveryPacket: "crates/modules/rustok-pages/docs/explicit-immutable-artifact-loss-activation-recovery.md",
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
  console.error("[verify-pages-explicit-artifact-binding-replacement] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);
if (evidence.format !== "pages_explicit_artifact_binding_replacement_source_v5") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_explicit_artifact_binding_replacement_repeated_loss_recovery_source_unvalidated") {
  failures.push("evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}
for (const value of Object.values(evidence.validation ?? {})) {
  if (value !== false) failures.push("evidence validation must remain unexecuted");
}
, marker, "latest actualization");
}

if (failures.length > 0) {
  console.error("[verify-pages-explicit-artifact-binding-replacement] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}
console.log("[verify-pages-explicit-artifact-binding-replacement] PASS");
