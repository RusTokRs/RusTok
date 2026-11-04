#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  limits: "crates/modules/rustok-page-builder/src/static_publish_resource_limits.rs",
  sanitization: "crates/modules/rustok-page-builder/src/publish_sanitization.rs",
  staticLanding: "crates/modules/rustok-page-builder/src/static_landing.rs",
  runtimeContract:
    "crates/modules/rustok-page-builder/contracts/page-builder-publish-runtime-review.json",
  evidence:
    "crates/modules/rustok-page-builder/contracts/evidence/page-builder-static-publish-resource-limits-source.json",
  packet: "crates/modules/rustok-page-builder/docs/static-publish-resource-limits.md",
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
  console.error("[verify-page-builder-static-publish-resource-limits] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);
const runtimeContract = JSON.parse(sources.runtimeContract);

if (evidence.format !== "page_builder_static_publish_resource_limits_source_v1") {
  failures.push("source evidence format drifted");
}
if (evidence.status !== "page_builder_static_publish_resource_limits_source_unvalidated") {
  failures.push("source evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("source evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`source evidence validation.${key} must remain false`);
}

const expectedLimits = {
  max_project_bytes: 16 * 1024 * 1024,
  max_pages: 128,
  max_components: 50_000,
  max_component_depth: 128,
  max_assets: 4_096,
  max_style_rules: 20_000,
};
for (const [key, value] of Object.entries(expectedLimits)) {
  if (evidence.source_contract?.[key] !== value) {
    failures.push(`source_contract.${key} drifted`);
  }
}
, marker, "resource-limit packet");

, marker, "parity actualization");

if (failures.length > 0) {
  console.error("[verify-page-builder-static-publish-resource-limits] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log(
  "[verify-page-builder-static-publish-resource-limits] PASS source_ready=true execution=pending",
);
