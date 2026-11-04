#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  harness: "crates/modules/rustok-pages/tests/explicit_artifact_repair_postgres.rs",
  evidence: "crates/modules/rustok-pages/contracts/evidence/pages-explicit-artifact-repair-postgres-source.json",
  reviewedPublish: "crates/modules/rustok-pages/src/services/page/reviewed_publish.rs",
  rebuildOwner: "crates/modules/rustok-pages/src/services/page/artifact_rebuild.rs",
  activationOwner: "crates/modules/rustok-pages/src/services/page/artifact_binding_replacement.rs",
  rebuildMigration: "crates/modules/rustok-pages/src/migrations/m20260806_000014_add_explicit_artifact_rebuild.rs",
  activationMigration: "crates/modules/rustok-pages/src/migrations/m20260807_000015_create_page_artifact_binding_replacements.rs",
  transactionalBus: "crates/modules/rustok-outbox/src/transactional.rs",
};
const absolute = (relative) => path.join(repoRoot, relative);
const read = (relative) => fs.readFileSync(absolute(relative), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const sliceBetween = (source, start, end, label) => {
  const startIndex = source.indexOf(start);
  if (startIndex < 0) {
    failures.push(`${label}: missing ${start}`);
    return "";
  }
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (endIndex < 0) {
    failures.push(`${label}: missing ${end}`);
    return "";
  }
  return source.slice(startIndex, endIndex);
};
const requireOrder = (source, markers, label) => {
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

for (const [label, relative] of Object.entries(files)) {
  if (!fs.existsSync(absolute(relative))) {
    failures.push(`${label}: missing regular file ${relative}`);
    continue;
  }
  const stats = fs.lstatSync(absolute(relative));
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${label}: ${relative} must be a regular non-symlink file`);
  }
}
if (failures.length > 0) {
  console.error("[verify-pages-explicit-artifact-repair-postgres] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relative]) => [label, read(relative)]),
);
const evidence = JSON.parse(sources.evidence);
const contract = evidence.source_contract ?? {};

if (evidence.format !== "pages_explicit_artifact_repair_postgres_source_v1") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_explicit_artifact_repair_postgres_source_unvalidated") {
  failures.push("evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
, marker, "PostgreSQL repair continuation");

if (failures.length > 0) {
  console.error("[verify-pages-explicit-artifact-repair-postgres] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-explicit-artifact-repair-postgres] PASS source_ready=true postgres_execution=pending");
