#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  migration:
    "crates/rustok-pages/src/migrations/m20260806_000013_create_page_publish_rebuild_sources.rs",
  migrations: "crates/rustok-pages/src/migrations/mod.rs",
  entity: "crates/rustok-pages/src/entities/page_publish_rebuild_source.rs",
  entities: "crates/rustok-pages/src/entities/mod.rs",
  publishManifest: "crates/rustok-pages/src/services/page/publish_manifest.rs",
  evidence:
    "crates/rustok-pages/contracts/evidence/pages-publish-rebuild-provenance-source.json",
  packet: "crates/rustok-pages/docs/immutable-artifact-rebuild-provenance.md",
  actualization: "docs/modules/page-builder-parity-actualization-2026-08-05.md",
  continuation:
    "docs/modules/pages-page-builder-rebuild-provenance-continuation-2026-08-06.md",
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
  console.error("[verify-pages-publish-rebuild-provenance] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);

if (evidence.format !== "pages_publish_rebuild_provenance_source_v1") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_publish_rebuild_provenance_source_unvalidated") {
  failures.push("evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`evidence validation.${key} must remain false`);
}
for (const key of [
  "captured_after_publish_operation_insert_in_same_transaction",
  "one_source_row_per_publish_operation_and_locale",
  "source_is_exact_sanitized_grapesjs_snapshot",
  "source_is_resanitized_and_integrity_verified",
  "sanitized_set_hash_is_recomputed_and_required",
  "artifact_set_hash_is_recomputed_and_required",
  "artifact_owner_and_locale_are_fenced",
  "provenance_survives_artifact_row_loss",
  "review_hash_is_retained",
  "materialization_hash_identity_and_runtime_snapshots_are_retained",
  "runtime_context_reauthorization_remains_required",
  "provenance_hash_binds_owner_source_review_artifact_and_runtime_hashes",
  "legacy_publish_operations_are_not_backfilled",
  "existing_artifacts_are_not_mutated",
  "bindings_are_not_changed",
  "automatic_repair_or_rebuild_is_not_added",
  "public_routes_and_cache_behavior_are_unchanged",
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`source_contract.${key} must be true`);
  }
}
for (const key of [
  "tests_run",
  "static_verifier_run",
  "cargo_run",
  "formatting_run",
  "database_migration_run",
  "publish_or_rebuild_run",
  "workflows_or_ci_run",
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`source_contract.${key} must remain false`);
  }
}

for (const marker of [
  "PagePublishRebuildSources::Table",
  "PagePublishRebuildSources::SanitizedProject",
  "PagePublishRebuildSources::ProvenanceHash",
  "idx_page_publish_rebuild_sources_operation_locale",
]) {
  need(sources.migration, marker, "migration");
}
need(
  sources.migrations,
  "m20260806_000013_create_page_publish_rebuild_sources",
  "migration registry",
);

for (const marker of [
  "fk_page_publish_rebuild_sources_artifact",
  "ForeignKeyAction::Restrict",
]) {
  forbid(sources.migration, marker, "migration");
}
for (const marker of [
  'table_name = "page_publish_rebuild_sources"',
  "pub operation_id: Uuid",
  "pub page_body_id: Uuid",
  "pub sanitized_project: Json",
  "pub materialization_identity: Json",
  "pub runtime_snapshots: Json",
  "pub provenance_hash: String",
]) {
  need(sources.entity, marker, "entity");
}
need(
  sources.entities,
  "pub mod page_publish_rebuild_source;",
  "entity registry",
);
need(
  sources.entities,
  "pub use page_publish_rebuild_source::Entity as PagePublishRebuildSource;",
  "entity export",
);

for (const marker of [
  'PAGE_PUBLISH_REBUILD_SOURCE_FORMAT: &str = "pages_publish_rebuild_source_v1"',
  "page_publish_rebuild_source::Entity::find()",
  "sanitize_static_landing_project(&project_data)",
  "sanitized.verify_integrity()",
  "artifact.materialization_hash.clone().ok_or_else",
  "artifact.materialization_identity.clone().ok_or_else",
  "artifact.runtime_snapshots.clone().ok_or_else",
  "artifact_manifest_hash != operation.artifact_set_hash",
  "sanitized_manifest_hash != operation.sanitized_set_hash",
  "page_publish_rebuild_source::ActiveModel",
  "provenance_hash: Set(row.provenance_hash)",
]) {
  need(sources.publishManifest, marker, "publish manifest");
}
for (const marker of [
  "page_static_landing_artifact::ActiveModel",
  "page_published_landing_artifact::ActiveModel",
  ".update(db)",
  ".delete(db)",
]) {
  forbid(sources.publishManifest, marker, "publish manifest");
}

for (const marker of [
  "page_publish_rebuild_sources",
  "same owner transaction",
  "Existing publish operations and legacy artifacts are not backfilled",
  "never update the damaged artifact in place",
  "automatic repair or rebuild is not added",
]) {
  need(sources.packet, marker, "packet");
}
for (const marker of [
  "publish-rebuild-provenance-source-ready",
  "Reviewed publish rebuild provenance",
  "repair/rebuild command remains open",
]) {
  need(sources.actualization, marker, "actualization");
}
for (const marker of [
  "source-ready / execution-pending",
  "immutable source provenance",
  "No automatic repair",
]) {
  need(sources.continuation, marker, "continuation");
}

if (failures.length > 0) {
  console.error("[verify-pages-publish-rebuild-provenance] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

console.log("[verify-pages-publish-rebuild-provenance] PASS");
