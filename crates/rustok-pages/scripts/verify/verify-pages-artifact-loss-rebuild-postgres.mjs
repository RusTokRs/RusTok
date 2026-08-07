#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const read = (relativePath) => fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
const failures = [];

const files = {
  evidence: "crates/rustok-pages/contracts/evidence/pages-artifact-loss-rebuild-postgres-source.json",
  harness: "crates/rustok-pages/tests/artifact_loss_rebuild_postgres.rs",
  reviewedPublish: "crates/rustok-pages/src/services/page/reviewed_publish.rs",
  rebuildOwner: "crates/rustok-pages/src/services/page/artifact_rebuild.rs",
  artifactOwner: "crates/rustok-pages/src/services/page_builder_artifact.rs",
  provenanceMigration: "crates/rustok-pages/src/migrations/m20260806_000013_create_page_publish_rebuild_sources.rs",
  rebuildMigration: "crates/rustok-pages/src/migrations/m20260806_000014_add_explicit_artifact_rebuild.rs",
  continuation: "docs/modules/pages-page-builder-artifact-loss-rebuild-continuation-2026-08-07.md",
};

for (const [label, relativePath] of Object.entries(files)) {
  const absolute = path.join(repoRoot, relativePath);
  if (!fs.existsSync(absolute)) {
    failures.push(`${label}: missing ${relativePath}`);
    continue;
  }
  const stats = fs.lstatSync(absolute);
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${label}: ${relativePath} must be a regular non-symlink file`);
  }
}
if (failures.length > 0) {
  console.error("[verify-pages-artifact-loss-rebuild-postgres] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);

const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const requireOrder = (source, markers, label) => {
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
const sliceBetween = (source, start, end, label) => {
  const startIndex = source.indexOf(start);
  if (startIndex < 0) {
    failures.push(`${label}: missing ${start}`);
    return "";
  }
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (endIndex < 0) {
    failures.push(`${label}: missing ${end}`);
    return "";
  }
  return source.slice(startIndex, endIndex);
};
const countText = (source, marker) => source.split(marker).length - 1;

if (evidence.format !== "pages_artifact_loss_rebuild_postgres_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_artifact_loss_rebuild_postgres_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const value of Object.values(evidence.validation ?? {})) {
  if (value !== false) failures.push("all validation flags must remain false");
}

for (const [key, expected] of Object.entries({
  postgres_environment_gated: true,
  isolated_postgres_schema_per_run: true,
  real_outbox_module_migrations_used: true,
  real_pages_module_migrations_used: true,
  real_page_service_publish_reviewed_used: true,
  reviewed_publish_revision_matches_owner_updated_at_snapshot: true,
  retained_provenance_captured_before_loss: true,
  canonical_artifact_model_captured_before_loss: true,
  binding_reference_removed_before_artifact_loss: true,
  publish_manifest_reference_removed_before_artifact_loss: true,
  source_artifact_row_deleted_before_rebuild: true,
  retained_provenance_survives_source_artifact_loss: true,
  provenance_migration_has_no_artifact_fk: true,
  rebuild_receipt_migration_has_no_source_artifact_fk: true,
  rebuild_owner_does_not_load_source_artifact_row: true,
  rebuild_uses_retained_provenance_after_source_artifact_loss: true,
  rebuilt_model_matches_pre_loss_canonical_model_except_storage_identity_and_created_at: true,
  rebuild_receipt_retains_missing_source_artifact_id_as_evidence: true,
  rebuild_appends_exactly_one_artifact: true,
  rebuild_appends_exactly_one_receipt: true,
  rebuild_preserves_retained_provenance: true,
  rebuild_does_not_recreate_binding: true,
  rebuild_preserves_page_version_and_status: true,
  rebuild_emits_no_lifecycle_events: true,
  exact_replay_reuses_operation_and_artifact: true,
  exact_replay_adds_no_artifact_receipt_or_event: true,
  automatic_repair_added: false,
  automatic_activation_added: false,
  production_code_changed: false,
  database_schema_changed: false,
  public_transport_changed: false,
  ffa_promoted: false,
  fba_promoted: false,
  tests_run: false,
  source_verifier_run: false,
  cargo_run: false,
  formatting_run: false,
  postgres_run: false,
  workflows_or_ci_run: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`source_contract.${key} must be ${expected}`);
  }
}

for (const marker of [
  'const DATABASE_ENV: &str = "RUSTOK_PAGES_TEST_DATABASE_URL"',
  'env::var("DATABASE_URL")',
  "CREATE SCHEMA",
  "DROP SCHEMA IF EXISTS",
  "OutboxModule",
  "PagesModule.migrations()",
  ".publish_reviewed(",
  "let canonical_artifact =",
  "let retained_source = source.clone();",
  "page_published_landing_artifact::Entity::delete_many()",
  "page_publish_operation_artifact::Entity::delete_many()",
  "page_static_landing_artifact::Entity::delete_by_id(source.artifact_id)",
  ".rebuild_immutable_artifact(",
  "let mut expected_rebuilt = canonical_artifact.clone();",
  "expected_rebuilt.id = rebuilt_record.id;",
  "expected_rebuilt.instance_key = rebuilt_record.instance_key.clone();",
  "expected_rebuilt.created_at = rebuilt_record.created_at.clone();",
  "assert_eq!(rebuilt_record, expected_rebuilt);",
  "assert_eq!(receipt.source_artifact_id, source.artifact_id);",
  "assert!(replay.replayed);",
]) need(sources.harness, marker, "artifact-loss rebuild harness");

requireOrder(sources.harness, [
  "let canonical_artifact =",
  "let page_before_loss =",
  "let events_before_loss =",
  "let removed_bindings =",
  "let removed_manifest =",
  "let deleted_artifact =",
  "provenance disappeared after artifact loss",
  "let artifact_count_before_rebuild =",
  "let receipt_count_before_rebuild =",
  "let events_before_rebuild =",
  "let rebuild_input = RebuildPageArtifactInput",
  ".rebuild_immutable_artifact(",
  "assert!(!rebuilt.replayed);",
  "let rebuilt_record =",
  "assert_eq!(rebuilt_record, expected_rebuilt);",
  "let receipt =",
  "provenance changed during rebuild",
  "let page_after_rebuild =",
  "let replay = service",
  "assert!(replay.replayed);",
], "artifact loss explicit rebuild ordering");

for (const marker of [
  "assert_eq!(removed_bindings.rows_affected, 1);",
  "assert_eq!(removed_manifest.rows_affected, 1);",
  "assert_eq!(deleted_artifact.rows_affected, 1);",
  "assert_eq!(rebuilt.source_artifact_id, source.artifact_id);",
  "artifact_count_before_rebuild + 1",
  "receipt_count_before_rebuild + 1",
  "assert_eq!(SysEvents::find().count(&db).await?, events_before_rebuild);",
]) need(sources.harness, marker, "artifact-loss rebuild durable boundaries");

requireOrder(sources.reviewedPublish, [
  "fn body_revision_snapshot(bodies: &[page_body::Model]) -> BodyRevisionSnapshot",
  ".map(|body| (body.locale.clone(), body.updated_at.to_string()))",
  "revisions.sort();",
], "reviewed publish revision owner");

const rebuildCommand = sliceBetween(
  sources.rebuildOwner,
  "pub async fn rebuild_immutable_artifact(",
  "async fn load_source_in_tx(",
  "explicit rebuild owner command",
);
requireOrder(rebuildCommand, [
  "let source = load_source_in_tx",
  "verify_source(&source)?;",
  "let compiled = compile_exact_rebuild(&source, &reviewed)?;",
  "PageBuilderArtifactService::append_rebuilt_in_tx(",
  "source_artifact_id: Set(source.artifact_id)",
  "page_artifact_rebuild_operation::ActiveModel",
  ".insert(&txn)",
  "txn.commit().await?;",
], "explicit rebuild retained-provenance ordering");
for (const marker of [
  "page_static_landing_artifact::Entity",
  "find_by_id(source.artifact_id)",
  "load_bound_artifact",
]) forbid(rebuildCommand, marker, "rebuild owner source-artifact independence");

const appendRebuilt = sliceBetween(
  sources.artifactOwner,
  "pub(crate) async fn append_rebuilt_in_tx(",
  "pub(crate) async fn bind_existing_body_in_tx(",
  "append rebuilt artifact owner",
);
requireOrder(appendRebuilt, [
  "compiled_materialization(compiled)?",
  "enforce_size_limits(&compiled.page)?;",
  "let instance_key = rebuild_artifact_instance_key(rebuild_operation_id);",
  "let artifact_id = Uuid::new_v4();",
  "page_static_landing_artifact::Entity::insert(artifact_model(",
  "verify_record(&stored)?;",
  "ensure_same_artifact(&stored, compiled)?;",
], "append rebuilt artifact ordering");

if (countText(sources.provenanceMigration, "ForeignKey::create()") !== 1) {
  failures.push("provenance migration must retain exactly one foreign key");
}
need(
  sources.provenanceMigration,
  '.to(PagePublishOperations::Table, PagePublishOperations::Id)',
  "provenance migration publish-operation FK",
);
forbid(
  sources.provenanceMigration,
  ".to(PageStaticLandingArtifacts::Table",
  "provenance migration artifact FK boundary",
);

const rebuildTable = sliceBetween(
  sources.rebuildMigration,
  ".table(PageArtifactRebuildOperations::Table)",
  "manager\n            .create_index(",
  "rebuild receipt migration table",
);
if (countText(rebuildTable, "ForeignKey::create()") !== 2) {
  failures.push("rebuild receipt migration must retain exactly two foreign keys");
}
for (const marker of [
  "PageArtifactRebuildOperations::SourceArtifactId",
  '.to(Pages::Table, Pages::Id)',
  '.to(PagePublishRebuildSources::Table, PagePublishRebuildSources::Id)',
]) need(rebuildTable, marker, "rebuild receipt migration");
forbid(
  rebuildTable,
  ".to(PageStaticLandingArtifacts::Table",
  "rebuild receipt source-artifact FK boundary",
);

for (const marker of [
  "artifact-loss-rebuild-postgres-harness-source-ready",
  "pages_artifact_loss_rebuild_postgres_source_unvalidated",
  "missing source artifact row",
  "exact pre-loss canonical model",
  "does not recreate the published binding",
  "exact replay",
  "intentionally not run",
]) need(sources.continuation, marker, "artifact-loss rebuild continuation");

for (const forbidden of [
  "replace_rebuilt_artifact_binding(",
  "audit_page_artifacts(",
  "rebuildPageArtifact",
  "activateRebuiltPageArtifact",
]) forbid(sources.harness, forbidden, "artifact-loss harness automatic/transport boundary");

if (failures.length > 0) {
  console.error("[verify-pages-artifact-loss-rebuild-postgres] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-artifact-loss-rebuild-postgres] PASS source_ready=true execution=pending");
