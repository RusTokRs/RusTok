#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..", "..", "..");
const read = (relativePath) =>
  fs.readFileSync(path.join(repoRoot, relativePath), "utf8");

const evidence = JSON.parse(
  read(
    "crates/modules/rustok-pages/contracts/evidence/pages-explicit-artifact-repair-cache-source.json",
  ),
);
const harness = read(
  "crates/modules/rustok-pages/tests/explicit_artifact_repair_cache_postgres.rs",
);
const reviewedPublish = read(
  "crates/modules/rustok-pages/src/services/page/reviewed_publish.rs",
);
const rebuildOwner = read(
  "crates/modules/rustok-pages/src/services/page/artifact_rebuild.rs",
);
const activationOwner = read(
  "crates/modules/rustok-pages/src/services/page/artifact_binding_replacement.rs",
);
const artifactOwner = read(
  "crates/modules/rustok-pages/src/services/page_builder_artifact.rs",
);
const cacheOwner = read("crates/modules/rustok-pages/src/cache_invalidation.rs");
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

if (evidence.format !== "pages_explicit_artifact_repair_cache_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_explicit_artifact_repair_cache_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
, marker, "repair cache continuation");
}

for (const forbidden of [
  "audit_page_artifacts(",
  "rebuildPageArtifact",
  "activateRebuiltPageArtifact",
]) {
  forbidText(harness, forbidden, "repair cache automatic/transport boundary");
}

if (failures.length > 0) {
  console.error("[verify-pages-explicit-artifact-repair-cache] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "[verify-pages-explicit-artifact-repair-cache] PASS source_ready=true execution=pending",
);
