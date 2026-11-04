#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  evidence:
    "crates/modules/rustok-page-builder/contracts/evidence/page-builder-authenticated-inline-edit-adapter-source.json",
  inline: "crates/modules/rustok-page-builder/storefront/src/inline_edit.rs",
  adapterGuard:
    "crates/modules/rustok-page-builder/scripts/verify/verify-page-builder-authenticated-inline-edit-adapter.mjs",
  executionPlan: "docs/modules/pages-page-builder-inline-edit-execution-plan.md",
};

const absolute = (relativePath) => path.join(root, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};

for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(absolute(relativePath))) failures.push(`${label}: missing ${relativePath}`);
}
if (failures.length > 0) {
  console.error("[verify-page-builder-inline-session-dom-boundary] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const evidence = JSON.parse(read(files.evidence));
const inline = read(files.inline);
const adapterGuard = read(files.adapterGuard);
const executionPlan = read(files.executionPlan);

if (evidence.format !== "page_builder_authenticated_inline_edit_adapter_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "page_builder_authenticated_inline_edit_adapter_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
, marker, "corrective packet");
for (const marker of [
  "session-dom-boundary-source-fixed",
  "inline-edit-session-dom-boundary-source-fixed",
  "no longer emits `data-inline-session`",
  "session DOM exposure: source-fixed, validation pending",
]) need(executionPlan, marker, "execution plan");

if (failures.length > 0) {
  console.error("[verify-page-builder-inline-session-dom-boundary] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log(
  "[verify-page-builder-inline-session-dom-boundary] PASS session_dom_exposure=false source_fixed=true execution=pending",
);
