#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  service: "crates/rustok-pages/src/services/page/artifact_binding_replacement.rs",
  bindingOwner: "crates/rustok-pages/src/services/page_builder_artifact.rs",
  test: "crates/rustok-pages/tests/artifact_loss_activation_recovery_postgres.rs",
  priorLossTest: "crates/rustok-pages/tests/artifact_loss_rebuild_postgres.rs",
  evidence: "crates/rustok-pages/contracts/evidence/pages-explicit-artifact-binding-replacement-source.json",
  packet: "crates/rustok-pages/docs/explicit-immutable-artifact-loss-activation-recovery.md",
  actualization: "docs/modules/pages-page-builder-activation-recovery-implementation-actualization-2026-08-07.md",
};

const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
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
  console.error("[verify-pages-artifact-loss-activation-recovery-postgres] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);

if (evidence.format !== "pages_explicit_artifact_binding_replacement_source_v2") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_explicit_artifact_binding_replacement_recovery_source_unvalidated") {
  failures.push("evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`evidence validation.${key} must remain false`);
}
for (const key of [
  "missing_binding_recovery_requires_source_artifact_absent",
  "missing_binding_recovery_requires_retained_source_body_identity",
  "missing_binding_recovery_requires_exact_source_publish_operation",
  "missing_binding_recovery_requires_publish_result_version_equal_current_expected",
  "existing_binding_mismatch_never_falls_back_to_recovery",
  "postgres_recovery_harness_source_ready",
  "postgres_success_recovery_case_source_ready",
  "postgres_source_artifact_present_rejection_source_ready",
  "postgres_stale_publish_version_rejection_source_ready",
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`evidence source_contract.${key} must be true`);
  }
}
for (const key of [
  "tests_run",
  "static_verifier_run",
  "cargo_run",
  "formatting_run",
  "migration_run",
  "database_scenario_run",
  "binding_replacement_run",
  "workflows_or_ci_run",
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`evidence source_contract.${key} must remain false`);
  }
}

for (const marker of [
  "let page_body_id = match binding",
  "Some(binding) =>",
  "binding.page_body_id != source.page_body_id",
  "binding.artifact_id != input.expected_current_artifact_id",
  "None =>",
  "ensure_missing_binding_recovery_in_tx",
  "page_static_landing_artifact::Entity::find_by_id(rebuild.source_artifact_id)",
  "source_artifact.is_some()",
  "page_body::Entity::find_by_id(source.page_body_id)",
  "page_publish_operation::Entity::find_by_id(source.operation_id)",
  "publish.id != rebuild.source_publish_operation_id",
  "publish.result_version != expected_version",
  "PageBuilderArtifactService::bind_existing_body_in_tx",
  "page_body_id: Set(page_body_id)",
  "DomainEvent::NodeUpdated",
  "DomainEvent::NodePublished",
]) {
  need(sources.service, marker, "recovery service");
}
for (const marker of [
  "sanitize_static_landing_project",
  "compile_materialized_static_landing",
  "append_rebuilt_in_tx",
  "page_body::Column::Content",
  "body.content",
  "delete_by_id(rebuild.source_artifact_id)",
  "PagesCacheInvalidationRuntime",
]) {
  forbid(sources.service, marker, "recovery boundary");
}
requireOrdered(
  sources.service,
  [
    "load_binding_for_update_in_tx",
    "let page_body_id = match binding",
    "ensure_missing_binding_recovery_in_tx",
    "load_replacement_artifact_in_tx",
    "PageBuilderArtifactService::bind_existing_body_in_tx",
    "DomainEvent::NodeUpdated",
    "DomainEvent::NodePublished",
    "page_artifact_binding_replacement_operation::ActiveModel",
    "txn.commit().await?",
  ],
  "recovery transaction ordering",
);

for (const marker of [
  "pub(crate) async fn bind_existing_body_in_tx",
  "match page_published_landing_artifact::Entity::find_by_id(body.id)",
  "None =>",
  "page_published_landing_artifact::ActiveModel",
]) {
  need(sources.bindingOwner, marker, "binding owner");
}

for (const marker of [
  "missing_binding_activation_recovers_after_physical_source_artifact_loss_on_postgres",
  "missing_binding_activation_rejects_when_source_artifact_still_exists_on_postgres",
  "missing_binding_activation_rejects_stale_source_publish_version_on_postgres",
  "remove_binding_manifest_and_source_artifact",
  "page_static_landing_artifact::Entity::delete_by_id(source.artifact_id)",
  "replace_rebuilt_artifact_binding",
  "assert_eq!(activated.version, fixture.page_version + 1)",
  "events_before_activation + 2",
  "assert!(replay.replayed)",
  "source artifact still exists",
  "advanced.version = Set(fixture.page_version + 1)",
  "expected_version: fixture.page_version + 1",
  "source publish version is stale",
  "RUSTOK_PAGES_TEST_DATABASE_URL",
]) {
  need(sources.test, marker, "PostgreSQL recovery harness source");
}

for (const marker of [
  "explicit_rebuild_reproduces_missing_source_artifact_from_retained_provenance_on_postgres",
  "page_published_landing_artifact::Entity::find()",
  "rebuild_immutable_artifact",
  "assert_eq!(SysEvents::find().count(&db).await?, events_before_rebuild)",
]) {
  need(sources.priorLossTest, marker, "prior artifact-loss rebuild packet");
}

for (const marker of [
  "Missing-binding recovery admission",
  "publish_operation.result_version == expected_version",
  "Existing-binding path remains strict",
  "source artifact still exists",
  "stale-source-publish-version",
]) {
  need(sources.packet, marker, "recovery packet");
}
for (const marker of [
  "Missing-binding activation after physical source-artifact loss",
  "Source-ready in this overlay",
  "Dedicated PostgreSQL execution pending",
  "source artifact still exists",
  "retained source publish `result_version`",
]) {
  need(sources.actualization, marker, "parity actualization");
}

if (failures.length > 0) {
  console.error("[verify-pages-artifact-loss-activation-recovery-postgres] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}
console.log("[verify-pages-artifact-loss-activation-recovery-postgres] PASS");
