#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  evidence:
    "crates/rustok-page-builder/contracts/evidence/page-builder-authenticated-inline-edit-adapter-source.json",
  inline: "crates/rustok-page-builder-storefront/src/inline_edit.rs",
  adapterGuard:
    "crates/rustok-page-builder/scripts/verify/verify-page-builder-authenticated-inline-edit-adapter.mjs",
  packet:
    "docs/modules/pages-page-builder-inline-session-dom-boundary-packet-2026-08-06.md",
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
const packet = read(files.packet);
const executionPlan = read(files.executionPlan);

if (evidence.format !== "page_builder_authenticated_inline_edit_adapter_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "page_builder_authenticated_inline_edit_adapter_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const key of [
  "authorization_proof_is_not_rendered_into_dom",
  "grant_session_is_not_rendered_into_dom",
  "dom_root_identity_uses_page_and_project_hash",
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`evidence source_contract.${key} must be true`);
  }
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("source evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`evidence validation.${key} must remain false`);
}

for (const marker of [
  "let root_id = inline_root_id(&grant);",
  "fn inline_root_id(grant: &AuthenticatedInlineEditGrant) -> String",
  '"fly-inline-{}-{}"',
  "dom_id(grant.page_id())",
  "grant.expected_project_hash().hex()",
  "inline_dom_identity_excludes_grant_session_and_authorization_proof",
  "assert!(!root_id.contains(grant.session_id()))",
  'assert!(!root_id.contains("signed-proof"))',
]) need(inline, marker, "inline source");
for (const marker of [
  "data-inline-session",
  "dom_id(grant.session_id())",
  "data-inline-proof",
]) forbid(inline, marker, "inline source");

for (const marker of [
  '"grant_session_is_not_rendered_into_dom"',
  '"dom_root_identity_uses_page_and_project_hash"',
  '"data-inline-session"',
  '"dom_id(grant.session_id())"',
  "session_dom_exposure=false",
]) need(adapterGuard, marker, "adapter guard");

for (const marker of [
  "source-fixed / maintainer-validation-pending",
  "grant session identifier",
  "Fly page id + expected project hash",
  "data-inline-session",
  "session remains available only inside the trusted Rust/WASM grant",
  "Browser evidence remains pending",
]) need(packet, marker, "corrective packet");
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
