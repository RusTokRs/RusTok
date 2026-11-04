#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  audit: "crates/modules/rustok-pages/src/services/page/artifact_integrity_audit.rs",
  pageModule: "crates/modules/rustok-pages/src/services/page/mod.rs",
  services: "crates/modules/rustok-pages/src/services/mod.rs",
  lib: "crates/modules/rustok-pages/src/lib.rs",
  evidence:
    "crates/modules/rustok-pages/contracts/evidence/pages-immutable-artifact-integrity-audit-source.json",
  packet: "crates/modules/rustok-pages/docs/immutable-artifact-integrity-audit.md",
};

const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
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
  console.error("[verify-pages-immutable-artifact-integrity-audit] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);
const contract = evidence.source_contract ?? {};

if (evidence.format !== "pages_immutable_artifact_integrity_audit_source_v1") {
  failures.push("source evidence format drifted");
}
if (evidence.status !== "pages_immutable_artifact_integrity_audit_source_unvalidated") {
  failures.push("source evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("source evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`source evidence validation.${key} must remain false`);
}

for (const [key, expected] of Object.entries({
  command: "PageService::audit_immutable_artifact_integrity",
  result_format: "pages_immutable_artifact_integrity_audit_v1",
  required_permission: "pages:manage",
  required_permission_scope: "all",
  default_max_records: 128,
  hard_max_records: 512,
  max_returned_findings: 64,
  audit_hash_algorithm: "sha256",
})) {
  if (contract[key] !== expected) failures.push(`source_contract.${key} drifted`);
}
, marker, "audit packet");

, marker, "parity actualization");

if (failures.length > 0) {
  console.error("[verify-pages-immutable-artifact-integrity-audit] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log(
  "[verify-pages-immutable-artifact-integrity-audit] PASS source_ready=true execution=pending",
);
