#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  migration: "crates/modules/rustok-pages/src/migrations/m20260806_000014_add_explicit_artifact_rebuild.rs",
  migrations: "crates/modules/rustok-pages/src/migrations/mod.rs",
  artifactEntity: "crates/modules/rustok-pages/src/entities/page_static_landing_artifact.rs",
  operationEntity: "crates/modules/rustok-pages/src/entities/page_artifact_rebuild_operation.rs",
  entities: "crates/modules/rustok-pages/src/entities/mod.rs",
  artifactService: "crates/modules/rustok-pages/src/services/page_builder_artifact.rs",
  rebuildService: "crates/modules/rustok-pages/src/services/page/artifact_rebuild.rs",
  pageServices: "crates/modules/rustok-pages/src/services/page/mod.rs",
  services: "crates/modules/rustok-pages/src/services/mod.rs",
  crateRoot: "crates/modules/rustok-pages/src/lib.rs",
  publishManifest: "crates/modules/rustok-pages/src/services/page/publish_manifest.rs",
  dto: "crates/modules/rustok-pages/src/dto/page.rs",
  dtoMod: "crates/modules/rustok-pages/src/dto/mod.rs",
  audit: "crates/modules/rustok-pages/src/services/page/artifact_integrity_audit.rs",
  regression: "crates/modules/rustok-pages/tests/explicit_artifact_rebuild_sqlite.rs",
  evidence: "crates/modules/rustok-pages/contracts/evidence/pages-explicit-artifact-rebuild-source.json",
  packet: "crates/modules/rustok-pages/docs/explicit-immutable-artifact-rebuild.md",
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
  console.error("[verify-pages-explicit-artifact-rebuild] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);

if (evidence.format !== "pages_explicit_artifact_rebuild_source_v1") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_explicit_artifact_rebuild_source_unvalidated") {
  failures.push("evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`evidence validation.${key} must remain false`);
}
, marker, "rebuild packet");
}
, marker, "parity actualization");
  need(sources.continuation, marker, "rebuild continuation");
}

if (failures.length > 0) {
  console.error("[verify-pages-explicit-artifact-rebuild] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}
console.log("[verify-pages-explicit-artifact-rebuild] PASS");
