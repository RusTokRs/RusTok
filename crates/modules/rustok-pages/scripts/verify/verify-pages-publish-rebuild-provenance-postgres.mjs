#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const read = (relativePath) => fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
const failures = [];

const evidence = JSON.parse(read(
  "crates/modules/rustok-pages/contracts/evidence/pages-publish-rebuild-provenance-postgres-source.json",
));
const harness = read("crates/modules/rustok-pages/tests/publish_rebuild_provenance_postgres.rs");
const documentOwner = read("crates/modules/rustok-pages/src/services/page/document.rs");
const reviewedPublish = read("crates/modules/rustok-pages/src/services/page/reviewed_publish.rs");
const publishManifest = read("crates/modules/rustok-pages/src/services/page/publish_manifest.rs");
const publishOperation = read("crates/modules/rustok-pages/src/entities/page_publish_operation.rs");
const migration = read(
  "crates/modules/rustok-pages/src/migrations/m20260806_000013_create_page_publish_rebuild_sources.rs",
);
const continuation = read(
);

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
const count = (content, value) => content.split(value).length - 1;

if (evidence.format !== "pages_publish_rebuild_provenance_postgres_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_publish_rebuild_provenance_postgres_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
, marker, "provenance continuation");

for (const forbidden of [
  "rebuildPageArtifact",
  "activateRebuiltPageArtifact",
  "auditPageArtifacts",
]) forbidText(harness, forbidden, "provenance transport/automatic-repair boundary");

if (failures.length > 0) {
  console.error("[verify-pages-publish-rebuild-provenance-postgres] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-publish-rebuild-provenance-postgres] PASS source_ready=true execution=pending");
