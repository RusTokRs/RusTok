#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");

const evidence = JSON.parse(read(
  "crates/modules/rustok-pages/contracts/evidence/pages-selected-immutable-artifact-source.json",
));
const harness = read(
  "crates/modules/rustok-pages/tests/selected_immutable_published_artifact_sqlite.rs",
);
const artifactService = read(
  "crates/modules/rustok-pages/src/services/page_builder_artifact.rs",
);
const nativeAdapter = read(
  "crates/modules/rustok-pages/storefront/src/transport/native_server_adapter.rs",
);
const packet = read(
);
const plan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
const localPlan = read("crates/modules/rustok-pages/docs/implementation-plan.md");
const failures = [];

const need = (text, marker, label) => {
  if (!text.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (text, marker, label) => {
  if (text.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const ordered = (text, markers, label) => {
  let at = -1;
  for (const marker of markers) {
    at = text.indexOf(marker, at + 1);
    if (at < 0) {
      failures.push(`${label}: missing or out of order ${marker}`);
      return;
    }
  }
};
const between = (text, start, end, label) => {
  const from = text.indexOf(start);
  const to = text.indexOf(end, from + start.length);
  if (from < 0 || to < 0) {
    failures.push(`${label}: unable to locate source slice`);
    return "";
  }
  return text.slice(from, to);
};

if (evidence.format !== "pages_selected_immutable_artifact_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_selected_immutable_artifact_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
, marker, "selected immutable artifact packet");
for (const marker of [
  "selected-immutable-artifact-source-ready",
  "Selected immutable artifact after draft mutation: source-ready",
  "current Fly body is not public render authority"
]) need(plan, marker, "canonical Pages/Page Builder plan");
for (const marker of [
  "selected immutable published artifact regression",
  "persisted draft body mutation",
  "current body content is not public render authority"
]) need(localPlan, marker, "Pages local plan");

forbid(harness, "Iggy", "Pages selected artifact harness");
forbid(packet, "IggyTransport", "Pages selected artifact packet");
forbid(harness, "redis::", "Pages selected artifact harness");
forbid(harness, 'cmd("SCAN")', "Pages selected artifact harness");
forbid(harness, 'cmd("KEYS")', "Pages selected artifact harness");

if (failures.length) {
  console.error("[verify-pages-selected-immutable-artifact] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log(
  "[verify-pages-selected-immutable-artifact] PASS source_ready=true execution=pending public_authority=immutable_binding",
);
