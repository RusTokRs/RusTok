#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  owner: "crates/modules/rustok-pages/src/services/page/artifact_integrity_audit.rs",
  gql: "crates/modules/rustok-pages/src/graphql/artifact_integrity_audit.rs",
  gqlMod: "crates/modules/rustok-pages/src/graphql/mod.rs",
  http: "crates/modules/rustok-pages/src/http/artifact_integrity_audit.rs",
  httpMod: "crates/modules/rustok-pages/src/http.rs",
  openapi: "crates/modules/rustok-pages/src/openapi.rs",
  evidence:
    "crates/modules/rustok-pages/contracts/evidence/pages-immutable-artifact-integrity-audit-transport-source.json",
  packet: "crates/modules/rustok-pages/docs/immutable-artifact-integrity-audit-transport.md",
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
  console.error("[verify-pages-immutable-artifact-integrity-audit-transport] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);
const contract = evidence.source_contract ?? {};

if (evidence.format !== "pages_immutable_artifact_integrity_audit_transport_source_v1") {
  failures.push("source evidence format drifted");
}
if (
  evidence.status !==
  "pages_immutable_artifact_integrity_audit_transport_source_unvalidated"
) {
  failures.push("source evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("source evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`source evidence validation.${key} must remain false`);
}

for (const [key, expected] of Object.entries({
  service_command: "PageService::audit_immutable_artifact_integrity",
  graphql_mutation: "auditPageArtifacts",
  http_method: "POST",
  http_path: "/api/admin/pages/{id}/artifacts/audit",
  required_permission: "pages:manage",
})) {
  if (contract[key] !== expected) failures.push(`source_contract.${key} drifted`);
}
, marker, "transport packet");

, marker, "parity actualization");

if (failures.length > 0) {
  console.error("[verify-pages-immutable-artifact-integrity-audit-transport] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log(
  "[verify-pages-immutable-artifact-integrity-audit-transport] PASS source_ready=true execution=pending",
);
