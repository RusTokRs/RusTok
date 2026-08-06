#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  migration: "crates/rustok-pages/src/migrations/m20260806_000014_add_explicit_artifact_rebuild.rs",
  migrations: "crates/rustok-pages/src/migrations/mod.rs",
  artifactEntity: "crates/rustok-pages/src/entities/page_static_landing_artifact.rs",
  operationEntity: "crates/rustok-pages/src/entities/page_artifact_rebuild_operation.rs",
  entities: "crates/rustok-pages/src/entities/mod.rs",
  artifactService: "crates/rustok-pages/src/services/page_builder_artifact.rs",
  rebuildService: "crates/rustok-pages/src/services/page/artifact_rebuild.rs",
  pageServices: "crates/rustok-pages/src/services/page/mod.rs",
  services: "crates/rustok-pages/src/services/mod.rs",
  crateRoot: "crates/rustok-pages/src/lib.rs",
  publishManifest: "crates/rustok-pages/src/services/page/publish_manifest.rs",
  dto: "crates/rustok-pages/src/dto/page.rs",
  dtoMod: "crates/rustok-pages/src/dto/mod.rs",
  audit: "crates/rustok-pages/src/services/page/artifact_integrity_audit.rs",
  regression: "crates/rustok-pages/tests/explicit_artifact_rebuild_sqlite.rs",
  evidence: "crates/rustok-pages/contracts/evidence/pages-explicit-artifact-rebuild-source.json",
  packet: "crates/rustok-pages/docs/explicit-immutable-artifact-rebuild.md",
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
  console.error("[verify-pages-explicit-artifact-rebuild] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);

if (evidence.format !== "pages_explicit_artifact_rebuild_source_v1") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_explicit_artifact_rebuild_source_unvalidated") {
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
  "requires_exact_source_id",
  "requires_expected_provenance_hash",
  "requires_explicit_reviewed_runtime_context",
  "retained_runtime_context_is_not_used_as_authority",
  "mutable_current_draft_is_not_read",
  "retained_sanitized_source_is_resanitized_and_verified",
  "runtime_context_hash_and_scenario_are_reauthorized",
  "artifact_source_artifact_and_materialization_hashes_must_reproduce",
  "materialization_identity_and_runtime_snapshots_must_reproduce_exactly",
  "rebuilt_artifact_is_a_new_row",
  "source_artifact_is_not_updated_or_deleted",
  "rebuild_receipt_is_idempotent",
  "published_binding_is_not_changed",
  "page_version_is_not_changed",
  "lifecycle_events_are_not_emitted",
  "cache_generations_are_not_rotated",
  "graphql_http_openapi_and_admin_ui_are_not_added",
  "binding_replacement_remains_separate",
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
  "rebuild_run",
  "workflows_or_ci_run",
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`evidence source_contract.${key} must remain false`);
  }
}

for (const marker of [
  "PageStaticLandingArtifacts::InstanceKey",
  '.default("canonical")',
  "PageArtifactRebuildOperations::Table",
  "ExpectedProvenanceHash",
  "ArtifactInstanceKey",
  "RebuiltArtifactId",
  "idx_page_artifact_rebuild_operations_idempotency",
  "idx_page_artifact_rebuild_operations_artifact",
  "Forward-only by design",
]) {
  need(sources.migration, marker, "migration");
}
requireOrdered(
  sources.migration,
  [
    "drop_index(",
    'name("idx_page_static_landing_artifacts_build")',
    "PageStaticLandingArtifacts::InstanceKey",
    "create_table(",
    "PageArtifactRebuildOperations::Table",
  ],
  "migration ordering",
);
need(sources.migrations, "mod m20260806_000014_add_explicit_artifact_rebuild;", "migration registry");
need(
  sources.migrations,
  "m20260806_000014_add_explicit_artifact_rebuild::Migration",
  "migration sequence",
);

need(sources.artifactEntity, "pub instance_key: String", "artifact entity");
for (const marker of [
  'table_name = "page_artifact_rebuild_operations"',
  "pub source_id: Uuid",
  "pub expected_provenance_hash: String",
  "pub artifact_instance_key: String",
  "pub source_artifact_id: Uuid",
  "pub rebuilt_artifact_id: Uuid",
]) {
  need(sources.operationEntity, marker, "operation entity");
}
need(sources.entities, "pub mod page_artifact_rebuild_operation;", "entity registry");
need(sources.entities, "PageArtifactRebuildOperation", "entity export");

for (const marker of [
  'CANONICAL_ARTIFACT_INSTANCE_KEY: &str = "canonical"',
  'REBUILD_ARTIFACT_INSTANCE_PREFIX: &str = "rebuild:"',
  "pub(crate) async fn append_rebuilt_in_tx",
  "rebuild_artifact_instance_key(rebuild_operation_id)",
  "page_static_landing_artifact::Entity::insert",
  "instance_key: Set(instance_key.to_string())",
  "Column::InstanceKey",
]) {
  need(sources.artifactService, marker, "artifact service");
}
forbid(sources.artifactService, "active.update(txn).await?; // rebuild", "artifact service");

for (const marker of [
  "pub async fn rebuild_immutable_artifact",
  "PAGE_ARTIFACT_REBUILD_IDEMPOTENCY_CONFLICT",
  "PAGE_ARTIFACT_REBUILD_SOURCE_INVALID",
  "PAGE_ARTIFACT_REBUILD_OPERATION_INTEGRITY",
  "enforce_tenant_wide_manage(&security)?",
  "input.source_id",
  "input.expected_provenance_hash",
  "reviewed.runtime_context_hash()",
  "sanitize_static_landing_project(&source.sanitized_project)",
  "compile_materialized_static_landing(sanitized.project_data(), runtime)",
  "materialization_identity != source.materialization_identity",
  "runtime_snapshots != source.runtime_snapshots",
  "PageBuilderArtifactService::append_rebuilt_in_tx",
  "page_artifact_rebuild_operation::ActiveModel",
  "txn.commit().await?",
]) {
  need(sources.rebuildService, marker, "rebuild service");
}
for (const marker of [
  "page_body::Entity",
  "page_published_landing_artifact",
  "bind_existing_body_in_tx",
  "event_bus",
  "DomainEvent",
  "NodeUpdated",
  "NodePublished",
  "PageTransition",
  "apply_transition",
]) {
  forbid(sources.rebuildService, marker, "rebuild boundary");
}
requireOrdered(
  sources.rebuildService,
  [
    "verify_source(&source)?",
    "compile_exact_rebuild(&source, &reviewed)?",
    "append_rebuilt_in_tx(",
    "page_artifact_rebuild_operation::ActiveModel",
    "txn.commit().await?",
  ],
  "rebuild transaction ordering",
);

need(sources.publishManifest, "pub(super) fn rebuild_source_provenance_hash", "provenance helper");
need(sources.dto, "pub struct RebuildPageArtifactInput", "rebuild input");
need(sources.dto, "pub struct RebuildPageArtifactResult", "rebuild result");
need(sources.dtoMod, "RebuildPageArtifactInput", "DTO export");
need(sources.pageServices, "mod artifact_rebuild;", "page service registry");
need(sources.pageServices, "PAGE_ARTIFACT_REBUILD_OPERATION_FORMAT", "page service export");
need(sources.services, "PAGE_ARTIFACT_REBUILD_OPERATION_FORMAT", "service export");
need(sources.crateRoot, "PageArtifactRebuildOperation", "crate entity export");
need(sources.crateRoot, "PagePublishRebuildSource", "crate provenance export");
need(sources.crateRoot, "PAGE_ARTIFACT_REBUILD_OPERATION_INTEGRITY", "crate rebuild code export");
need(sources.audit, "record.instance_key", "audit instance identity");
need(sources.audit, "is_valid_artifact_instance_key", "audit instance validation");

for (const marker of [
  "explicit_rebuild_appends_exact_artifact_without_switching_public_binding",
  "Mutable draft must not become rebuild authority",
  "corrupted retained artifact",
  "rebuild_immutable_artifact(",
  "assert_eq!(binding_after.artifact_id, binding_before.artifact_id)",
  "assert_eq!(page_after.version, page_before.version)",
  "assert!(replay.replayed)",
]) {
  need(sources.regression, marker, "SQLite regression");
}

for (const marker of [
  "Explicit Immutable Artifact Rebuild",
  "tenant-wide `pages:manage`",
  "rebuild:<rebuild-operation-uuid>",
  "does not decide that the new row should become public",
  "Binding replacement remains a separate",
]) {
  need(sources.packet, marker, "rebuild packet");
}
for (const marker of [
  "explicit-artifact-rebuild-source-ready",
  "Explicit append-only repair/rebuild command",
  "Explicit binding replacement",
]) {
  need(sources.actualization, marker, "parity actualization");
  need(sources.continuation, marker, "rebuild continuation");
}

if (failures.length > 0) {
  console.error("[verify-pages-explicit-artifact-rebuild] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}
console.log("[verify-pages-explicit-artifact-rebuild] PASS");
