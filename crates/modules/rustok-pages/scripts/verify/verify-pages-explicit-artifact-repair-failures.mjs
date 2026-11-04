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
    "crates/modules/rustok-pages/contracts/evidence/pages-explicit-artifact-repair-failures-source.json",
  ),
);
const harness = read(
  "crates/modules/rustok-pages/tests/explicit_artifact_repair_failures_sqlite.rs",
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
const continuation = read(
);
const postgresContinuation = read(
);
const actualization = read(
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
const sliceBetween = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  if (startIndex < 0) {
    failures.push(`${label}: missing ${start}`);
    return "";
  }
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (endIndex < 0) {
    failures.push(`${label}: missing ${end}`);
    return "";
  }
  return content.slice(startIndex, endIndex);
};
const countText = (content, value) => content.split(value).length - 1;

if (evidence.status !== "pages_explicit_artifact_repair_failures_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
, marker, "failure continuation");
}
, marker, "canonical parity actualization");
}

for (const forbidden of [
  "audit_page_artifacts(",
  "rebuildPageArtifact",
  "activateRebuiltPageArtifact",
]) {
  forbidText(harness, forbidden, "failure harness automatic/transport boundary");
}

if (failures.length > 0) {
  console.error("[verify-pages-explicit-artifact-repair-failures] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "[verify-pages-explicit-artifact-repair-failures] PASS source_ready=true execution=pending",
);
