#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  migration: "crates/rustok-pages/src/migrations/m20260807_000015_create_page_artifact_binding_replacements.rs",
  migrations: "crates/rustok-pages/src/migrations/mod.rs",
  entity: "crates/rustok-pages/src/entities/page_artifact_binding_replacement_operation.rs",
  entities: "crates/rustok-pages/src/entities/mod.rs",
  dto: "crates/rustok-pages/src/dto/artifact_binding_replacement.rs",
  dtoMod: "crates/rustok-pages/src/dto/mod.rs",
  service: "crates/rustok-pages/src/services/page/artifact_binding_replacement.rs",
  artifactService: "crates/rustok-pages/src/services/page_builder_artifact.rs",
  pageServices: "crates/rustok-pages/src/services/page/mod.rs",
  services: "crates/rustok-pages/src/services/mod.rs",
  lib: "crates/rustok-pages/src/lib.rs",
  test: "crates/rustok-pages/tests/explicit_artifact_binding_replacement_sqlite.rs",
  singleLossTest: "crates/rustok-pages/tests/artifact_loss_activation_recovery_postgres.rs",
  multiLossTest: "crates/rustok-pages/tests/artifact_loss_multilocale_activation_recovery_postgres.rs",
  evidence: "crates/rustok-pages/contracts/evidence/pages-explicit-artifact-binding-replacement-source.json",
  packet: "crates/rustok-pages/docs/explicit-immutable-artifact-binding-replacement.md",
  recoveryPacket: "crates/rustok-pages/docs/explicit-immutable-artifact-loss-activation-recovery.md",
  actualization: "docs/modules/pages-page-builder-activation-recovery-implementation-actualization-2026-08-07.md",
  multiActualization: "docs/modules/pages-page-builder-multilocale-activation-recovery-actualization-2026-08-07.md",
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

if (evidence.format !== "pages_explicit_artifact_binding_replacement_source_v3") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_explicit_artifact_binding_replacement_multilocale_recovery_source_unvalidated") {
  failures.push("evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`evidence validation.${key} must remain false`);
}
for (const key of [
  "requires_tenant_wide_pages_manage",
  "requires_exact_rebuild_operation_id",
  "requires_expected_page_version",
  "requires_expected_current_artifact_id",
  "rebuild_receipt_integrity_is_verified",
  "rebuild_provenance_source_is_revalidated",
  "rebuild_receipt_must_match_provenance_source",
  "existing_binding_path_requires_exact_body_and_artifact",
  "existing_binding_mismatch_never_falls_back_to_recovery",
  "missing_binding_recovery_is_explicit_only",
  "missing_binding_recovery_requires_source_artifact_absent",
  "missing_binding_recovery_requires_retained_source_body_identity",
  "missing_binding_recovery_requires_exact_source_publish_operation",
  "missing_binding_first_recovery_accepts_publish_result_version_equal_current_expected",
  "missing_binding_sequential_recovery_requires_contiguous_activation_version_chain",
  "missing_binding_sequential_recovery_requires_same_source_publish",
  "missing_binding_sequential_recovery_requires_other_unique_locales",
  "missing_binding_sequential_recovery_revalidates_prior_rebuild_and_provenance",
  "missing_binding_sequential_recovery_recomputes_prior_activation_request_hash",
  "missing_binding_sequential_recovery_requires_prior_repaired_binding_still_active",
  "missing_binding_sequential_recovery_requires_prior_rebuilt_artifact_identity",
  "missing_binding_sequential_recovery_rejects_unexplained_version_gap",
  "missing_binding_sequential_recovery_rejects_prior_activation_for_target_locale",
  "missing_binding_sequential_recovery_is_bounded",
  "missing_binding_recovery_reuses_bind_existing_body",
  "missing_binding_recovery_does_not_recreate_source_artifact",
  "replacement_artifact_owner_locale_instance_and_hashes_are_verified",
  "replacement_artifact_full_integrity_is_verified_before_binding_update",
  "only_one_locale_binding_is_updated_per_command",
  "source_artifact_is_not_updated_or_deleted",
  "replacement_artifact_is_not_recompiled_or_mutated",
  "mutable_current_draft_content_is_not_used_as_repair_authority",
  "page_must_remain_published",
  "page_version_advances_once_per_activation",
  "node_updated_and_node_published_are_written_transactionally",
  "cache_invalidation_is_event_driven_after_commit",
  "replacement_receipt_is_idempotent",
  "one_activation_receipt_is_allowed_per_rebuild",
  "graphql_http_openapi_admin_ui_and_workers_are_not_added",
  "automatic_audit_to_repair_is_not_added",
  "automatic_rebuild_to_activation_is_not_added",
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
  "PageArtifactBindingReplacementOperations::Table",
  "RebuildOperationId",
  "ExpectedVersion",
  "ExpectedCurrentArtifactId",
  "ReplacementArtifactId",
  "ReplacementArtifactHash",
  "ReplacementMaterializationHash",
  "ResultVersion",
  "idx_page_artifact_binding_replacements_idempotency",
  "idx_page_artifact_binding_replacements_rebuild",
  "idx_page_artifact_binding_replacements_result",
  "fk_page_artifact_binding_replacements_rebuild",
]) {
  need(sources.migration, marker, "migration");
}
need(sources.migrations, "mod m20260807_000015_create_page_artifact_binding_replacements;", "migration registry");
need(sources.migrations, "m20260807_000015_create_page_artifact_binding_replacements::Migration", "migration sequence");

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
need(sources.entities, "pub mod page_artifact_binding_replacement_operation;", "entity registry");
need(sources.entities, "PageArtifactBindingReplacementOperation", "entity export");

for (const marker of [
  "pub struct ReplacePageArtifactBindingInput",
  "pub rebuild_operation_id: Uuid",
  "pub expected_version: i32",
  "pub expected_current_artifact_id: Uuid",
  "pub idempotency_key: String",
  "pub struct ReplacePageArtifactBindingResult",
  "pub previous_artifact_id: Uuid",
  "pub replacement_artifact_id: Uuid",
]) {
  need(sources.dto, marker, "replacement DTO");
}
need(sources.dtoMod, "pub mod artifact_binding_replacement;", "DTO registry");
need(sources.dtoMod, "ReplacePageArtifactBindingInput", "DTO export");

for (const marker of [
  "pub async fn replace_rebuilt_artifact_binding",
  "enforce_tenant_wide_manage(&security)?",
  "find_page_for_update(&txn, tenant_id, page_id)",
  "enforce_expected_version(Some(input.expected_version), existing_page.version)?",
  'existing_page.status != "published"',
  "verify_rebuild_receipt(&rebuild)?",
  "load_rebuild_source_in_tx",
  "verify_rebuild_source(&source)?",
  "ensure_rebuild_matches_source(&rebuild, &source)?",
  "rebuild.source_artifact_id != input.expected_current_artifact_id",
  "find_operation_for_rebuild_in_tx",
  "load_binding_for_update_in_tx",
  "let page_body_id = match binding",
  "Some(binding) =>",
  "binding.page_body_id != source.page_body_id",
  "binding.artifact_id != input.expected_current_artifact_id",
  "None =>",
  "ensure_missing_binding_recovery_in_tx",
  "source_artifact.is_some()",
  "page_body::Entity::find_by_id(source.page_body_id)",
  "page_publish_operation::Entity::find_by_id(source.operation_id)",
  "publish.id != rebuild.source_publish_operation_id",
  "publish.result_version > expected_version",
  "publish.result_version < expected_version",
  "ensure_sequential_missing_binding_recovery_version_chain_in_tx",
  "MAX_SEQUENTIAL_RECOVERY_ACTIVATIONS",
  "operations.len() != version_gap",
  "operation.expected_version != cursor",
  "operation.result_version != cursor + 1",
  "operation.locale == target_locale",
  "prior_locales.insert(operation.locale.clone())",
  "operation.request_hash != expected_request_hash",
  "prior_rebuild.source_publish_operation_id != publish.id",
  "prior_source.operation_id != publish.id",
  "prior_binding.artifact_id != prior_rebuild.rebuilt_artifact_id",
  "prior_artifact.instance_key != prior_rebuild.artifact_instance_key",
  "cursor != expected_version",
  "load_replacement_artifact_in_tx",
  "replacement.instance_key != rebuild.artifact_instance_key",
  "PageBuilderArtifactService::bind_existing_body_in_tx",
  "active.version = Set(active.version.take().unwrap_or(1) + 1)",
  "DomainEvent::NodeUpdated",
  "DomainEvent::NodePublished",
  "page_body_id: Set(page_body_id)",
  "page_artifact_binding_replacement_operation::ActiveModel",
  "txn.commit().await?",
]) {
  need(sources.service, marker, "replacement service");
}

for (const marker of [
  "sanitize_static_landing_project",
  "compile_materialized_static_landing",
  "append_rebuilt_in_tx",
  "page_body::Column::Content",
  "body.content",
  "replace_current_published_set_in_tx",
  "delete_many()",
  "PagesCacheInvalidationRuntime",
  "GraphQL",
  "OpenAPI",
  "axum",
]) {
  forbid(sources.service, marker, "replacement boundary");
}
requireOrdered(
  sources.service,
  [
    "find_page_for_update(&txn, tenant_id, page_id)",
    "enforce_expected_version(Some(input.expected_version), existing_page.version)?",
    "verify_rebuild_receipt(&rebuild)?",
    "load_rebuild_source_in_tx",
    "verify_rebuild_source(&source)?",
    "ensure_rebuild_matches_source(&rebuild, &source)?",
    "load_binding_for_update_in_tx",
    "load_replacement_artifact_in_tx",
    "PageBuilderArtifactService::bind_existing_body_in_tx",
    "DomainEvent::NodeUpdated",
    "DomainEvent::NodePublished",
    "page_artifact_binding_replacement_operation::ActiveModel",
    "txn.commit().await?",
  ],
  "replacement transaction ordering",
);
requireOrdered(
  sources.service,
  [
    "publish.result_version > expected_version",
    "publish.result_version < expected_version",
    "ensure_sequential_missing_binding_recovery_version_chain_in_tx",
  ],
  "missing-binding version admission",
);

for (const marker of [
  "pub(crate) async fn bind_existing_body_in_tx",
  "page_body::Entity::find()",
  "page_published_landing_artifact::Entity::find_by_id(body.id)",
  "page_published_landing_artifact::ActiveModel",
]) {
  need(sources.artifactService, marker, "binding owner");
}

need(sources.pageServices, "mod artifact_binding_replacement;", "page service registry");
need(sources.pageServices, "PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_FORMAT", "page service export");
need(sources.services, "PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_FORMAT", "service export");
need(sources.lib, "PageArtifactBindingReplacementOperation", "crate entity export");
need(sources.lib, "PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_FORMAT", "crate service export");

for (const marker of [
  "explicit_binding_replacement_switches_exact_rebuild_and_replays",
  "corrupted current artifact",
  "PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT",
  "replace_rebuilt_artifact_binding",
  "assert_eq!(binding_after.artifact_id, rebuild.rebuilt_artifact_id)",
  "assert_eq!(page_after.version, page_before.version + 1)",
  "assert!(replay.replayed)",
]) {
  need(sources.test, marker, "existing-binding SQLite regression source");
}
for (const marker of [
  "missing_binding_activation_recovers_after_physical_source_artifact_loss_on_postgres",
  "missing_binding_activation_rejects_when_source_artifact_still_exists_on_postgres",
  "missing_binding_activation_rejects_stale_source_publish_version_on_postgres",
  "source publish version is stale",
]) {
  need(sources.singleLossTest, marker, "single-locale PostgreSQL recovery source");
}
for (const marker of [
  "missing_binding_activation_recovers_two_lost_locales_sequentially_on_postgres",
  "missing_binding_activation_rejects_unexplained_version_between_locales_on_postgres",
  "expected_version: en_activation.version",
  "not fully explained",
]) {
  need(sources.multiLossTest, marker, "multi-locale PostgreSQL recovery source");
}

for (const marker of [
  "Explicit Immutable Artifact Binding Replacement",
  "tenant-wide `pages:manage`",
  "expected current artifact",
  "retained provenance",
  "NodeUpdated",
  "NodePublished",
  "one activation receipt",
]) {
  need(sources.packet, marker, "replacement packet");
}
for (const marker of [
  "Explicit Immutable Artifact-Loss Activation Recovery",
  "Existing-binding path remains strict",
  "Missing-binding recovery admission",
  "publish_operation.result_version == expected_version",
  "Sequential multi-locale version chain",
  "unexplained lifecycle/version increment",
  "does not recreate the missing canonical source artifact",
  "automatic audit-to-rebuild",
]) {
  need(sources.recoveryPacket, marker, "recovery packet");
}
for (const marker of [
  "missing-binding-activation-recovery-source-ready",
  "Existing-binding path",
  "Missing-binding physical-loss path",
  "Source-ready in this overlay",
  "Dedicated PostgreSQL execution pending",
  "FFA/FBA promotion",
]) {
  need(sources.actualization, marker, "single-locale parity actualization");
}
for (const marker of [
  "Multi-Locale Artifact-Loss Activation Recovery Actualization",
  "multilocale-missing-binding-recovery-source-ready",
  "same-publish activation chain",
  "Unexplained version drift remains fail-closed",
]) {
  need(sources.multiActualization, marker, "multi-locale parity actualization");
}

if (failures.length > 0) {
  console.error("[verify-pages-explicit-artifact-binding-replacement] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}
console.log("[verify-pages-explicit-artifact-binding-replacement] PASS");
