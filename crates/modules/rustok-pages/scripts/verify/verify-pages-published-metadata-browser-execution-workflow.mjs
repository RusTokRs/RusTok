#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
  "..",
  "..",
);
const failures = [];
const files = {
  contract:
    "crates/modules/rustok-pages/contracts/evidence/pages-published-metadata-browser-execution-contract.json",
  surfaceEvidence:
    "crates/modules/rustok-pages/contracts/evidence/pages-published-metadata-surface-source.json",
  workflow: ".github/workflows/pages-published-metadata-browser-evidence.yml",
  sourceExecution:
    "crates/modules/rustok-pages/contracts/evidence/pages-consumer-properties-source-execution.json",
  sourceWorkflow: ".github/workflows/pages-consumer-properties-source-evidence.yml",
};
const verifier =
  "crates/modules/rustok-pages/scripts/verify/verify-pages-published-metadata-browser-execution-workflow.mjs";

const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
const requireValue = (condition, message) => {
  if (!condition) failures.push(message);
};
const requireText = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbidText = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const requireOrderedMarkers = (source, markers, label) => {
  let previous = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker, previous + 1);
    if (index < 0) {
      failures.push(`${label}: missing or out of order at ${marker}`);
      return;
    }
    previous = index;
  }
};

, marker, "published metadata browser actualization");
}

if (failures.length > 0) {
  console.error("[verify-pages-published-metadata-browser-execution-workflow] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "[verify-pages-published-metadata-browser-execution-workflow] PASS dispatch_only=true reviewed_deployment_required=true protected_storage_state=true run_index=ready browser_execution=pending",
);
