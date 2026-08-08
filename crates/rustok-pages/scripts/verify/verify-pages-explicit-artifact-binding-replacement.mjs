#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  migration: "crates/rustok-pages/src/migrations/m20260807_000015_create_page_artifact_binding_replacements.rs",
  entity: "crates/rustok-pages/src/entities/page_artifact_binding_replacement_operation.rs",
  dto: "crates/rustok-pages/src/dto/artifact_binding_replacement.rs",
  service: "crates/rustok-pages/src/services/page/artifact_binding_replacement.rs",
  rollback: "crates/rustok-pages/src/services/page/rollback.rs",
  artifactService: "crates/rustok-pages/src/services/page_builder_artifact.rs",
  sqliteTest: "crates/rustok-pages/tests/explicit_artifact_binding_replacement_sqlite.rs",
  singleLossTest: "crates/rustok-pages/tests/artifact_loss_activation_recovery_postgres.rs",
  multiLossTest: "crates/rustok-pages/tests/artifact_loss_multilocale_activation_recovery_postgres.rs",
  rollbackLossTest: "crates/rustok-pages/tests/artifact_loss_after_rollback_activation_recovery_postgres.rs",
  repeatedLossTest: "crates/rustok-pages/tests/artifact_repeated_loss_recovery_postgres.rs",
  evidence: "crates/rustok-pages/contracts/evidence/pages-explicit-artifact-binding-replacement-source.json",
  recoveryPacket: "crates/rustok-pages/docs/explicit-immutable-artifact-loss-activation-recovery.md",
  latestOverlay: "docs/modules/pages-page-builder-repeated-artifact-loss-recovery-actualization-2026-08-07.md",
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
  console.error("[verify-pages-explicit-artifact-binding-replacement] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);
if (evidence.format !== "pages_explicit_artifact_binding_replacement_source_v5") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_explicit_artifact_binding_replacement_repeated_loss_recovery_source_unvalidated") {
  failures.push("evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}
for (const value of Object.values(evidence.validation ?? {})) {
  if (value !== false) failures.push("evidence validation must remain unexecuted");
}
for (const key of [
  "requires_tenant_wide_pages_manage",
  "requires_exact_rebuild_operation_id",
  "requires_expected_page_version",
  "requires_expected_current_artifact_id",
  "expected_current_artifact_id_remains_historical_source_identity_on_missing_binding_recovery",
  "rebuild_receipt_integrity_is_verified",
  "rebuild_provenance_source_is_revalidated",
  "existing_binding_path_requires_exact_body_and_artifact",
  "existing_binding_mismatch_never_falls_back_to_recovery",
  "missing_binding_recovery_requires_source_artifact_absent",
  "missing_binding_recovery_requires_retained_source_body_identity",
  "missing_binding_direct_publish_anchor_accepts_publish_result_version_equal_current_expected",
  "missing_binding_rollback_activation_anchor_is_supported",
  "missing_binding_sequential_recovery_requires_contiguous_activation_version_chain",
  "missing_binding_sequential_recovery_requires_same_source_publish",
  "missing_binding_sequential_recovery_tracks_latest_repair_state_per_locale",
  "missing_binding_sequential_recovery_allows_repeated_locale_only_after_prior_rebuilt_artifact_absence",
  "missing_binding_sequential_recovery_requires_latest_non_target_binding_still_active",
  "missing_binding_sequential_recovery_requires_latest_non_target_rebuilt_artifact_identity",
  "missing_binding_sequential_recovery_requires_target_binding_absent",
  "missing_binding_sequential_recovery_requires_latest_target_prior_rebuilt_artifact_absent_before_repeat",
  "missing_binding_sequential_recovery_rejects_unexplained_version_gap",
  "replacement_artifact_full_integrity_is_verified_before_binding_update",
  "page_version_advances_once_per_activation",
  "node_updated_and_node_published_are_written_transactionally",
  "replacement_receipt_is_idempotent",
  "one_activation_receipt_is_allowed_per_rebuild",
  "postgres_repeated_loss_recovery_harness_source_ready",
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
  "RebuildOperationId",
  "ExpectedVersion",
  "ExpectedCurrentArtifactId",
  "ReplacementArtifactId",
  "ResultVersion",
  "idx_page_artifact_binding_replacements_rebuild",
  "idx_page_artifact_binding_replacements_result",
  "fk_page_artifact_binding_replacements_rebuild",
]) {
  need(sources.migration, marker, "migration");
}
for (const marker of [
  'table_name = "page_artifact_binding_replacement_operations"',
  "pub rebuild_operation_id: Uuid",
  "pub expected_version: i32",
  "pub expected_current_artifact_id: Uuid",
  "pub replacement_artifact_id: Uuid",
  "pub result_version: i32",
]) {
  need(sources.entity, marker, "receipt entity");
}
for (const marker of [
  "pub struct ReplacePageArtifactBindingInput",
  "pub rebuild_operation_id: Uuid",
  "pub expected_version: i32",
  "pub expected_current_artifact_id: Uuid",
  "pub idempotency_key: String",
  "pub struct ReplacePageArtifactBindingResult",
]) {
  need(sources.dto, marker, "replacement DTO");
}

for (const marker of [
  "pub async fn replace_rebuilt_artifact_binding",
  "enforce_tenant_wide_manage(&security)?",
  "find_page_for_update(&txn, tenant_id, page_id)",
  "enforce_expected_version(Some(input.expected_version), existing_page.version)?",
  "verify_rebuild_receipt(&rebuild)?",
  "load_rebuild_source_in_tx",
  "ensure_rebuild_matches_source(&rebuild, &source)?",
  "rebuild.source_artifact_id != input.expected_current_artifact_id",
  "find_operation_for_rebuild_in_tx",
  "load_binding_for_update_in_tx",
  "ensure_missing_binding_recovery_in_tx",
  "source_artifact.is_some()",
  "page_publish_operation::Entity::find_by_id(source.operation_id)",
  "resolve_missing_binding_recovery_anchor_in_tx",
  "PAGE_ROLLBACK_ACTIVATION_ANCHOR_FORMAT",
  "ensure_sequential_missing_binding_recovery_version_chain_in_tx",
  "MAX_SEQUENTIAL_RECOVERY_ACTIVATIONS",
  "let mut latest_by_locale",
  "operation.request_hash != expected_request_hash",
  "recovery_artifact_if_present_in_tx",
  "a repeated locale still has its prior rebuilt immutable artifact",
  "target locale binding unexpectedly became active before repeated recovery",
  "target locale prior rebuilt immutable artifact still exists",
  "latest repaired locale binding is no longer active",
  "latest repaired immutable artifact drifted from its rebuild receipt",
  "load_replacement_artifact_in_tx",
  "PageBuilderArtifactService::bind_existing_body_in_tx",
  "DomainEvent::NodeUpdated",
  "DomainEvent::NodePublished",
  "page_artifact_binding_replacement_operation::ActiveModel",
  "txn.commit().await?",
]) {
  need(sources.service, marker, "replacement service");
}
requireOrdered(sources.service, [
  "find_page_for_update(&txn, tenant_id, page_id)",
  "enforce_expected_version(Some(input.expected_version), existing_page.version)?",
  "verify_rebuild_receipt(&rebuild)?",
  "load_rebuild_source_in_tx",
  "load_binding_for_update_in_tx",
  "load_replacement_artifact_in_tx",
  "PageBuilderArtifactService::bind_existing_body_in_tx",
  "page_artifact_binding_replacement_operation::ActiveModel",
  "txn.commit().await?",
], "replacement transaction ordering");
for (const marker of [
  "sanitize_static_landing_project",
  "compile_materialized_static_landing",
  "append_rebuilt_in_tx",
  "page_body::Column::Content",
  "body.content",
  "replace_current_published_set_in_tx",
  "PagesCacheInvalidationRuntime",
  "GraphQL",
  "OpenAPI",
  "axum",
]) {
  forbid(sources.service, marker, "replacement boundary");
}
need(sources.rollback, 'const PAGE_ROLLBACK_OPERATION_FORMAT: &str = "page_rollback_operation_v1"', "rollback request format owner");
need(sources.artifactService, "pub(crate) async fn bind_existing_body_in_tx", "binding owner");

for (const marker of [
  "explicit_binding_replacement_switches_exact_rebuild_and_replays",
  "assert!(replay.replayed)",
]) {
  need(sources.sqliteTest, marker, "SQLite source packet");
}
for (const marker of [
  "missing_binding_activation_recovers_after_physical_source_artifact_loss_on_postgres",
  "RUSTOK_PAGES_TEST_DATABASE_URL",
]) {
  need(sources.singleLossTest, marker, "single-loss PostgreSQL packet");
}
for (const marker of [
  "missing_binding_activation_recovers_two_lost_locales_sequentially_on_postgres",
  "missing_binding_activation_rejects_unexplained_version_between_locales_on_postgres",
]) {
  need(sources.multiLossTest, marker, "multi-loss PostgreSQL packet");
}
for (const marker of [
  "rollback_activated_publish_recovers_two_lost_locales_sequentially_on_postgres",
  "rollback_activated_recovery_rejects_noncanonical_rollback_anchor_hash_on_postgres",
]) {
  need(sources.rollbackLossTest, marker, "rollback-activated PostgreSQL packet");
}
for (const marker of [
  "missing_binding_activation_recovers_same_locale_after_rebuilt_artifact_is_lost_again_on_postgres",
  "repeated_locale_recovery_rejects_missing_binding_while_prior_rebuilt_artifact_still_exists_on_postgres",
  "another_locale_can_recover_after_repeated_locale_loss_on_postgres",
  "rollback_continues_after_same_locale_is_recovered_twice_on_postgres",
]) {
  need(sources.repeatedLossTest, marker, "repeated-loss PostgreSQL packet");
}
for (const marker of [
  "Sequential multi-locale and repeated-loss version chain",
  "latest repair state per locale",
  "prior rebuilt instance is physically absent",
]) {
  need(sources.recoveryPacket, marker, "recovery packet");
}
for (const marker of [
  "Repeated Artifact-Loss Recovery Actualization",
  "latest-state-per-locale",
  "execution remains pending",
]) {
  need(sources.latestOverlay, marker, "latest actualization");
}

if (failures.length > 0) {
  console.error("[verify-pages-explicit-artifact-binding-replacement] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}
console.log("[verify-pages-explicit-artifact-binding-replacement] PASS");
