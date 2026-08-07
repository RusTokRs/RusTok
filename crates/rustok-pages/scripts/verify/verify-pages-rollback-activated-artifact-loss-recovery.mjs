#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  activation: "crates/rustok-pages/src/services/page/artifact_binding_replacement.rs",
  rollback: "crates/rustok-pages/src/services/page/rollback.rs",
  test: "crates/rustok-pages/tests/artifact_loss_after_rollback_activation_recovery_postgres.rs",
  evidence: "crates/rustok-pages/contracts/evidence/pages-rollback-activated-artifact-loss-recovery-source.json",
  priorOverlay: "docs/modules/pages-page-builder-multilocale-rollback-evidence-actualization-2026-08-07.md",
  currentOverlay: "docs/modules/pages-page-builder-rollback-activated-recovery-actualization-2026-08-07.md",
  pagesPlan: "crates/rustok-pages/docs/implementation-plan.md",
  fba: "crates/rustok-page-builder/contracts/page-builder-fba-registry.json",
};

const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
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
  console.error("[verify-pages-rollback-activated-artifact-loss-recovery] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);

if (evidence.format !== "pages_rollback_activated_artifact_loss_recovery_source_v1") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_rollback_activated_artifact_loss_recovery_source_unvalidated") {
  failures.push("evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}
for (const value of Object.values(evidence.validation ?? {})) {
  if (value !== false) failures.push("evidence validation fields must remain false");
}
for (const key of [
  "direct_publish_result_version_remains_a_valid_recovery_anchor",
  "rollback_anchor_requires_exact_target_publish_operation",
  "rollback_anchor_requires_exact_target_artifact_set_hash",
  "rollback_anchor_requires_distinct_source_and_target_artifact_sets",
  "rollback_anchor_requires_canonical_request_hash",
  "rollback_anchor_request_expected_version_is_result_version_minus_one",
  "rollback_anchor_must_not_be_newer_than_current_expected_version",
  "latest_matching_rollback_anchor_is_selected",
  "missing_matching_rollback_anchor_falls_back_to_original_publish_anchor",
  "post_anchor_version_gap_requires_only_contiguous_same_publish_activation_receipts",
  "post_anchor_activation_scan_is_physically_bounded_to_257_rows",
  "post_anchor_prior_locales_must_be_unique_and_other",
  "post_anchor_activation_request_hashes_are_recomputed",
  "post_anchor_rebuild_and_provenance_are_revalidated",
  "post_anchor_prior_bindings_must_remain_active",
  "post_anchor_rebuilt_artifact_identity_is_revalidated",
  "source_artifact_must_still_be_physically_absent",
  "retained_source_body_identity_is_still_required",
  "rollback_receipt_is_not_repair_source_authority",
  "historical_rollback_target_rules_are_unchanged",
  "automatic_rollback_to_repair_chaining_is_not_added",
  "postgres_two_locale_recovery_after_rollback_source_ready",
  "postgres_noncanonical_rollback_anchor_hash_rejection_source_ready",
  "postgres_unexplained_post_rollback_version_drift_rejection_source_ready",
  "schema_migration_dto_transport_cache_or_ui_changes_are_not_added",
]) {
  if (evidence.source_contract?.[key] !== true) failures.push(`source_contract.${key} must be true`);
}
for (const key of [
  "tests_run",
  "static_verifier_run",
  "cargo_run",
  "formatting_run",
  "migration_run",
  "database_scenario_run",
  "workflows_or_ci_run",
]) {
  if (evidence.source_contract?.[key] !== false) failures.push(`source_contract.${key} must remain false`);
}

for (const marker of [
  "page_rollback_operation",
  "PAGE_ROLLBACK_ACTIVATION_ANCHOR_FORMAT",
  "resolve_missing_binding_recovery_anchor_in_tx",
  "TargetPublishOperationId.eq(publish.id)",
  "TargetArtifactSetHash",
  "ResultVersion.lte(expected_version)",
  "order_by_desc(page_rollback_operation::Column::ResultVersion)",
  "rollback.result_version.checked_sub(1)",
  "rollback.request_hash != expected_request_hash",
  "return Ok(publish.result_version)",
  "anchor_version",
  ".gt(anchor_version)",
  ".limit((MAX_SEQUENTIAL_RECOVERY_ACTIVATIONS + 1) as u64)",
  "prior_locales.insert(operation.locale.clone())",
  "operation.request_hash != expected_request_hash",
  "prior_rebuild.source_publish_operation_id != publish.id",
  "prior_binding.artifact_id != prior_rebuild.rebuilt_artifact_id",
  "prior_artifact.instance_key != prior_rebuild.artifact_instance_key",
]) {
  need(sources.activation, marker, "activation source");
}
for (const marker of [
  "const PAGE_ROLLBACK_OPERATION_FORMAT: &str = \"page_rollback_operation_v1\"",
  "target_publish_operation_id",
  "target_artifact_set_hash",
  "result_version",
  "rollback_request_hash",
]) {
  need(sources.rollback, marker, "rollback owner source");
}
need(sources.activation, 'PAGE_ROLLBACK_ACTIVATION_ANCHOR_FORMAT: &str = "page_rollback_operation_v1"', "shared rollback hash format lock");

for (const marker of [
  "rollback_activated_publish_recovers_two_lost_locales_sequentially_on_postgres",
  "rollback_activated_recovery_rejects_noncanonical_rollback_anchor_hash_on_postgres",
  "rollback_activated_recovery_rejects_unexplained_version_drift_on_postgres",
  "rollback_to_previous",
  "remove_source_artifact",
  "UPDATE page_rollback_operations SET request_hash = $1 WHERE id = $2",
  "UPDATE pages SET version = $1 WHERE id = $2 AND tenant_id = $3",
  "RUSTOK_PAGES_TEST_DATABASE_URL",
]) {
  need(sources.test, marker, "PostgreSQL source packet");
}
for (const marker of [
  "Adjacent source gap: physical loss after rollback activation",
  "The next source slice must introduce an exact activation anchor",
]) {
  need(sources.priorOverlay, marker, "prior overlay");
}
for (const marker of [
  "Rollback-Activated Artifact-Loss Recovery Actualization",
  "Exact rollback activation anchor",
  "Unexplained post-rollback version drift remains rejected",
  "Execution remains separate and pending",
]) {
  need(sources.currentOverlay, marker, "current overlay");
}
for (const marker of [
  "rollback-activated-artifact-loss-recovery-source-ready",
  "publish-or-exact-rollback activation anchor",
  "artifact_loss_after_rollback_activation_recovery_postgres.rs",
  "verify-pages-rollback-activated-artifact-loss-recovery.mjs",
]) {
  need(sources.pagesPlan, marker, "Pages implementation plan");
}
for (const marker of [
  "rollback_activation_anchor_supported",
  "rollback_anchor_request_hash_recomputed",
  "rollback_activated_recovery_test",
  "pages_rollback_activated_artifact_loss_recovery_verifier",
]) {
  need(sources.fba, marker, "Page Builder FBA registry");
}
for (const marker of [
  "sanitize_static_landing_project",
  "compile_materialized_static_landing",
  "PagesCacheInvalidationRuntime",
  "GraphQL",
  "OpenAPI",
]) {
  forbid(sources.activation, marker, "activation recovery boundary");
}

if (failures.length > 0) {
  console.error("[verify-pages-rollback-activated-artifact-loss-recovery] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}
console.log("[verify-pages-rollback-activated-artifact-loss-recovery] PASS");
