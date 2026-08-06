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
  pageServices: "crates/rustok-pages/src/services/page/mod.rs",
  services: "crates/rustok-pages/src/services/mod.rs",
  lib: "crates/rustok-pages/src/lib.rs",
  test: "crates/rustok-pages/tests/explicit_artifact_binding_replacement_sqlite.rs",
  evidence: "crates/rustok-pages/contracts/evidence/pages-explicit-artifact-binding-replacement-source.json",
  packet: "crates/rustok-pages/docs/explicit-immutable-artifact-binding-replacement.md",
  actualization: "docs/modules/page-builder-parity-actualization-2026-08-05.md",
  continuation: "docs/modules/pages-page-builder-rebuild-provenance-continuation-2026-08-06.md",
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

if (evidence.format !== "pages_explicit_artifact_binding_replacement_source_v1") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_explicit_artifact_binding_replacement_source_unvalidated") {
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
  "current_binding_must_equal_rebuild_source_artifact",
  "replacement_artifact_owner_locale_instance_and_hashes_are_verified",
  "replacement_artifact_full_integrity_is_verified_before_binding_update",
  "only_one_locale_binding_is_updated",
  "source_artifact_is_not_updated_or_deleted",
  "replacement_artifact_is_not_recompiled_or_mutated",
  "mutable_current_draft_is_not_read",
  "page_must_remain_published",
  "page_version_advances_once",
  "node_updated_and_node_published_are_written_transactionally",
  "cache_invalidation_is_event_driven_after_commit",
  "replacement_receipt_is_idempotent",
  "one_activation_receipt_is_allowed_per_rebuild",
  "graphql_http_openapi_admin_ui_and_workers_are_not_added",
  "automatic_audit_to_repair_is_not_added",
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
need(
  sources.migrations,
  "mod m20260807_000015_create_page_artifact_binding_replacements;",
  "migration registry",
);
need(
  sources.migrations,
  "m20260807_000015_create_page_artifact_binding_replacements::Migration",
  "migration sequence",
);

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
need(
  sources.entities,
  "pub mod page_artifact_binding_replacement_operation;",
  "entity registry",
);
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
  "rebuild.source_artifact_id != input.expected_current_artifact_id",
  "find_operation_for_rebuild_in_tx",
  "load_binding_for_update_in_tx",
  "binding.artifact_id != input.expected_current_artifact_id",
  "load_replacement_artifact_in_tx",
  "replacement.instance_key != rebuild.artifact_instance_key",
  "PageBuilderArtifactService::bind_existing_body_in_tx",
  "active.version = Set(active.version.take().unwrap_or(1) + 1)",
  "DomainEvent::NodeUpdated",
  "DomainEvent::NodePublished",
  "page_artifact_binding_replacement_operation::ActiveModel",
  "txn.commit().await?",
]) {
  need(sources.service, marker, "replacement service");
}
for (const marker of [
  "sanitize_static_landing_project",
  "compile_materialized_static_landing",
  "append_rebuilt_in_tx",
  "page_body::Entity",
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

need(sources.pageServices, "mod artifact_binding_replacement;", "page service registry");
need(
  sources.pageServices,
  "PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_FORMAT",
  "page service export",
);
need(
  sources.services,
  "PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_FORMAT",
  "service export",
);
need(sources.lib, "PageArtifactBindingReplacementOperation", "crate entity export");
need(
  sources.lib,
  "PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_FORMAT",
  "crate service export",
);

for (const marker of [
  "explicit_binding_replacement_switches_exact_rebuild_and_replays",
  "corrupted current artifact",
  "PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT",
  "replace_rebuilt_artifact_binding",
  "assert_eq!(binding_after.artifact_id, rebuild.rebuilt_artifact_id)",
  "assert_eq!(page_after.version, page_before.version + 1)",
  "assert!(public.document_html.contains(\"Explicit binding replacement\"))",
  "assert!(replay.replayed)",
]) {
  need(sources.test, marker, "SQLite regression source");
}

for (const marker of [
  "Explicit Immutable Artifact Binding Replacement",
  "tenant-wide `pages:manage`",
  "expected current artifact",
  "NodeUpdated",
  "NodePublished",
  "one activation receipt",
  "does not",
]) {
  need(sources.packet, marker, "replacement packet");
}
for (const marker of [
  "explicit-artifact-binding-replacement-source-ready",
  "Explicit binding replacement",
  "tenant-admin repair transports",
]) {
  need(sources.actualization, marker, "parity actualization");
  need(sources.continuation, marker, "rebuild continuation");
}

if (failures.length > 0) {
  console.error("[verify-pages-explicit-artifact-binding-replacement] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}
console.log("[verify-pages-explicit-artifact-binding-replacement] PASS");
