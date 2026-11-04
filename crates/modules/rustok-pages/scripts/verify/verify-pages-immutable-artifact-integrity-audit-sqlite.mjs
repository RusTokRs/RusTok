#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..", "..", "..");
const read = (relativePath) => fs.readFileSync(path.join(repoRoot, relativePath), "utf8");

const evidence = JSON.parse(read(
  "crates/modules/rustok-pages/contracts/evidence/pages-immutable-artifact-integrity-audit-sqlite-source.json",
));
const harness = read("crates/modules/rustok-pages/tests/immutable_artifact_integrity_audit_sqlite.rs");
const owner = read("crates/modules/rustok-pages/src/services/page/artifact_integrity_audit.rs");
const reviewedPublish = read("crates/modules/rustok-pages/src/services/page/reviewed_publish.rs");
const continuation = read(
);
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireOrder = (content, values, label) => {
  let previous = -1;
  for (const value of values) {
    const index = content.indexOf(value, previous + 1);
    if (index < 0) {
      failures.push(`${label}: missing or out of order ${value}`);
      return;
    }
    previous = index;
  }
};

if (evidence.status !== "pages_immutable_artifact_integrity_audit_sqlite_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
, marker, "audit SQLite continuation");

for (const forbidden of [
  "rebuildPageArtifact",
  "activateRebuiltPageArtifact",
  "audit_page_artifacts(",
]) forbidText(harness, forbidden, "audit harness transport/automatic-repair boundary");

if (failures.length > 0) {
  console.error("[verify-pages-immutable-artifact-integrity-audit-sqlite] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-immutable-artifact-integrity-audit-sqlite] PASS source_ready=true execution=pending");
