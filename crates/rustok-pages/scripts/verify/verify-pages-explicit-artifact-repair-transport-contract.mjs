#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  test: "crates/rustok-pages/tests/explicit_artifact_repair_transport_contract.rs",
  evidence: "crates/rustok-pages/contracts/evidence/pages-explicit-artifact-repair-transport-contract-source.json",
  continuation: "docs/modules/pages-page-builder-rebuild-provenance-continuation-2026-08-06.md",
};
const read = (relative) => fs.readFileSync(path.join(repoRoot, relative), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};

for (const [label, relative] of Object.entries(files)) {
  const absolute = path.join(repoRoot, relative);
  if (!fs.existsSync(absolute) || !fs.lstatSync(absolute).isFile()) {
    failures.push(`${label}: missing regular file ${relative}`);
  }
}
if (failures.length > 0) {
  console.error("[verify-pages-explicit-artifact-repair-transport-contract] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const test = read(files.test);
const evidenceSource = read(files.evidence);
const continuation = read(files.continuation);
const evidence = JSON.parse(evidenceSource);

if (evidence.format !== "pages_explicit_artifact_repair_transport_contract_source_v1") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_explicit_artifact_repair_transport_contract_source_unvalidated") {
  failures.push("evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
for (const key of [
  "tests_run",
  "source_verifier_run",
  "cargo_run",
  "formatting_run",
  "graphql_or_http_run",
  "openapi_generation_run",
  "database_scenario_run",
  "workflows_or_ci_run",
]) {
  if (evidence.source_contract?.[key] !== false) failures.push(`source_contract.${key} must remain false`);
}

for (const marker of [
  "Schema::build(",
  "PagesQuery::default()",
  "PagesMutation::default()",
  "rebuildPageArtifact(",
  "activateRebuiltPageArtifact(",
  "GqlRebuildPageArtifactResult",
  "GqlActivateRebuiltPageArtifactResult",
  "rustok_pages::openapi::openapi_document()",
  "/api/admin/pages/{id}/artifacts/rebuild",
  "/api/admin/pages/{id}/artifacts/activate",
  "RebuildPageArtifactTransportResult",
  "ActivateRebuiltPageArtifactTransportResult",
  "RebuildPageArtifactInput",
  "ReplacePageArtifactBindingInput",
]) need(test, marker, "contract test");

for (const marker of [
  '"sourceId:"',
  '"sourcePublishOperationId:"',
  '"artifactInstanceKey:"',
  '"idempotencyKey:"',
  '"materializationIdentity:"',
  '"runtimeSnapshots:"',
  '"source_id"',
  '"source_publish_operation_id"',
  '"artifact_instance_key"',
  '"idempotency_key"',
  '"materialization_identity"',
  '"runtime_snapshots"',
]) need(test, marker, "bounded-field assertions");

for (const marker of [
  "Database::connect",
  "Entity::find",
  "rebuild_immutable_artifact(",
  "replace_rebuilt_artifact_binding(",
  "tower::ServiceExt",
  "oneshot(",
]) forbid(test, marker, "contract test");

need(continuation, "explicit-artifact-repair-transport-contract-harness-source-ready", "continuation");
need(continuation, "explicit_artifact_repair_transport_contract", "continuation");
need(continuation, "intentionally not run", "continuation");

if (failures.length > 0) {
  console.error("[verify-pages-explicit-artifact-repair-transport-contract] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-explicit-artifact-repair-transport-contract] PASS source_ready=true execution=pending");
