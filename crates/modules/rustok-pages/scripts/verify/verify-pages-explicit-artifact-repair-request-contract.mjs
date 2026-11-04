#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  test: "crates/modules/rustok-pages/tests/explicit_artifact_repair_request_contract.rs",
  evidence: "crates/modules/rustok-pages/contracts/evidence/pages-explicit-artifact-repair-request-contract-source.json",
  transportDoc: "crates/modules/rustok-pages/docs/explicit-artifact-repair-transports.md",
  auditDoc: "crates/modules/rustok-pages/docs/immutable-artifact-integrity-audit.md",
  auditTransportDoc: "crates/modules/rustok-pages/docs/immutable-artifact-integrity-audit-transport.md",
  rebuildDoc: "crates/modules/rustok-pages/docs/explicit-immutable-artifact-rebuild.md",
  activationDoc: "crates/modules/rustok-pages/docs/explicit-immutable-artifact-binding-replacement.md",
};
const absolute = (relative) => path.join(repoRoot, relative);
const read = (relative) => fs.readFileSync(absolute(relative), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};

for (const [label, relative] of Object.entries(files)) {
  if (!fs.existsSync(absolute(relative))) {
    failures.push(`${label}: missing ${relative}`);
    continue;
  }
  const stats = fs.lstatSync(absolute(relative));
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${label}: ${relative} must be a regular non-symlink file`);
  }
}
if (failures.length > 0) {
  console.error("[verify-pages-explicit-artifact-repair-request-contract] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relative]) => [label, read(relative)]),
);
const evidence = JSON.parse(sources.evidence);
const contract = evidence.source_contract ?? {};

if (evidence.format !== "pages_explicit_artifact_repair_request_contract_source_v1") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_explicit_artifact_repair_request_contract_source_unvalidated") {
  failures.push("evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
, marker, "request continuation");

for (const [label, source] of [
  ["transport doc", sources.transportDoc],
  ["audit doc", sources.auditDoc],
  ["audit transport doc", sources.auditTransportDoc],
  ["rebuild doc", sources.rebuildDoc],
  ["activation doc", sources.activationDoc],
  ["prior continuation", sources.priorContinuation],
  ["rebuild continuation", sources.rebuildContinuation],
  ["parity actualization", sources.actualization],
]) {
  need(source, "PermissionScope::All", label);
  need(source, "PermissionScope::None", label);
}

for (const [label, source, marker] of [
  [
    "transport doc",
    sources.transportDoc,
    "An owner-scoped Manage grant may pass the adapter's coarse permission check but is rejected by the owner command before writes.",
  ],
  [
    "audit doc",
    sources.auditDoc,
    "An owner-scoped `pages:manage` permission does not authorize reading retained page-wide artifacts.",
  ],
  [
    "audit transport doc",
    sources.auditTransportDoc,
    "owner-scoped Manage rejection by the service",
  ],
  ["rebuild doc", sources.rebuildDoc, "Owner-scoped Manage is insufficient."],
  ["activation doc", sources.activationDoc, "Owner-scoped Manage is insufficient."],
  [
    "rebuild continuation",
    sources.rebuildContinuation,
    "tenant-wide Manage succeeds and owner-scoped Manage rejects",
  ],
  [
    "parity actualization",
    sources.actualization,
    "tenant-wide versus owner-scoped Manage",
  ],
]) forbid(source, marker, label);

, marker, "parity actualization");

if (failures.length > 0) {
  console.error("[verify-pages-explicit-artifact-repair-request-contract] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-explicit-artifact-repair-request-contract] PASS source_ready=true execution=pending");
