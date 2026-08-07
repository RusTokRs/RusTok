#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const read = (relativePath) => fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
const failures = [];

const evidence = JSON.parse(read(
  "crates/rustok-pages/contracts/evidence/pages-publish-rebuild-provenance-postgres-source.json",
));
const harness = read("crates/rustok-pages/tests/publish_rebuild_provenance_postgres.rs");
const reviewedPublish = read("crates/rustok-pages/src/services/page/reviewed_publish.rs");
const publishManifest = read("crates/rustok-pages/src/services/page/publish_manifest.rs");
const publishOperation = read("crates/rustok-pages/src/entities/page_publish_operation.rs");
const migration = read(
  "crates/rustok-pages/src/migrations/m20260806_000013_create_page_publish_rebuild_sources.rs",
);
const continuation = read(
  "docs/modules/pages-page-builder-publish-provenance-postgres-continuation-2026-08-07.md",
);

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireOrder = (content, values, label) => {
  let previous = -1;
  for (const value of values) {
    const index = content.indexOf(value, previous + 1);
    if (index < 0) {
      failures.push(`${label}: missing or out of order ${value}`);
      return;
    }
    previous = index;
  }
};
const count = (content, value) => content.split(value).length - 1;

if (evidence.format !== "pages_publish_rebuild_provenance_postgres_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_publish_rebuild_provenance_postgres_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const key of [
  "source_guard",
  "postgres_harness",
  "two_locale_exact_capture",
  "artifact_set_mismatch_rollback",
  "sanitized_set_mismatch_rollback",
  "artifact_row_loss_survivability",
  "migration_no_backfill_source_boundary",
  "migration_no_artifact_fk_source_boundary",
]) {
  if (evidence.validation?.[key] !== false) failures.push(`validation.${key} must remain false`);
}
for (const [key, expected] of Object.entries({
  postgres_environment_gated: true,
  isolated_postgres_schema_per_run: true,
  real_outbox_module_migrations_used: true,
  real_pages_module_migrations_used: true,
  real_page_service_create_used: true,
  real_page_service_save_document_used: true,
  real_page_service_publish_reviewed_used: true,
  reviewed_publish_revision_matches_owner_updated_at_snapshot: true,
  two_locale_reviewed_publish_source_present: true,
  locale_ordered_rebuild_sources_expected: true,
  source_revision_matches_exact_body_updated_at: true,
  source_body_identity_bound: true,
  source_artifact_identity_bound: true,
  source_review_hash_bound: true,
  source_sanitized_artifact_materialization_hashes_bound: true,
  artifact_set_hash_mismatch_exercises_publish_operation_after_save: true,
  sanitized_set_hash_mismatch_exercises_publish_operation_after_save: true,
  aggregate_mismatch_rolls_back_publish_operation: true,
  aggregate_mismatch_adds_no_manifest_rows: true,
  aggregate_mismatch_adds_no_rebuild_source_rows: true,
  artifact_loss_fixture_removes_binding_and_manifest_references_first: true,
  artifact_row_can_be_deleted_without_deleting_rebuild_source: true,
  rebuild_source_survives_artifact_row_loss_unchanged: true,
  migration_rebuild_source_fk_targets_publish_operation_only: true,
  migration_has_no_artifact_fk: true,
  migration_has_no_legacy_backfill_statement: true,
  production_code_changed: false,
  database_schema_changed: false,
  public_transport_changed: false,
  automatic_repair_added: false,
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
  ".create(",
  ".save_document(",
  ".publish_reviewed(",
  'locale: "en".to_string()',
  'locale: "fr".to_string()',
  'expected_revision: format!("page:{}:initial", draft.id)',
]) requireText(harness, marker, "provenance PostgreSQL harness foundation");

requireOrder(harness, [
  "let en_revision = draft",
  ".updated_at",
  ".clone();",
  "let fr_saved = service",
  ".save_document(",
  "let fr_revision = fr_saved",
  ".updated_at",
  ".clone();",
  ".publish_reviewed(",
  "revision: fr_revision.clone()",
  "revision: en_revision.clone()",
], "two-locale reviewed publish revision ordering");
requireOrder(reviewedPublish, [
  "fn body_revision_snapshot(bodies: &[page_body::Model]) -> BodyRevisionSnapshot",
  ".map(|body| (body.locale.clone(), body.updated_at.to_string()))",
  "revisions.sort();",
], "reviewed publish owner revision snapshot");

requireOrder(harness, [
  "let sources = page_publish_rebuild_source::Entity::find()",
  ".order_by_asc(page_publish_rebuild_source::Column::Locale)",
  "assert_eq!(sources.len(), 2);",
  'vec!["en", "fr"]',
  "assert_source_matches_published_locale(",
], "locale-ordered exact source capture");
for (const marker of [
  "assert_eq!(source.source_revision, expected_revision);",
  "assert_eq!(body.updated_at.to_string(), source.source_revision);",
  "assert_eq!(artifact.source_hash, source.source_hash);",
  "assert_eq!(artifact.artifact_hash, source.artifact_hash);",
  "Some(source.materialization_hash.as_str())",
  "Some(&source.materialization_identity)",
  "Some(&source.runtime_snapshots)",
  "assert_eq!(source.review_hash, expected_review_hash);",
  "assert_eq!(source.provenance_hash.len(), 64);",
]) requireText(harness, marker, "exact provenance locale binding");

requireOrder(harness, [
  "AggregateMismatch::ArtifactSet",
  "AggregateMismatch::SanitizedSet",
  "assert_provenance_survives_artifact_row_loss",
], "aggregate rollback then artifact-loss ordering");
requireOrder(harness, [
  "let txn = db.begin().await?;",
  "let fake_operation_id = Uuid::new_v4();",
  "active.id = Set(fake_operation_id);",
  ".insert(&txn)",
  'expect_err("mismatched publish aggregate must reject receipt persistence")',
  "txn.rollback().await?;",
  "find_by_id(fake_operation_id)",
  "Column::OperationId.eq(fake_operation_id)",
], "aggregate mismatch transaction rollback");
for (const marker of [
  'message.contains("artifact_set_hash")',
  'message.contains("sanitized_set_hash")',
  "operation_count_before",
  "source_count_before",
  "manifest_count_before",
]) requireText(harness, marker, "aggregate mismatch zero-side-effect snapshot");

requireOrder(publishOperation, [
  "async fn after_save<C>(model: Model, db: &C, insert: bool)",
  "if insert",
  "persist_publish_manifest_after_save(",
  "PUBLISH_MANIFEST_DB_ERROR_PREFIX",
], "publish operation after-save provenance hook");
requireOrder(publishManifest, [
  "let artifact_manifest_hash = stable_hash(",
  "if artifact_manifest_hash != operation.artifact_set_hash",
  "let sanitized_manifest_hash = stable_hash(",
  "if sanitized_manifest_hash != operation.sanitized_set_hash",
  "for row in rows",
  "page_publish_operation_artifact::ActiveModel",
  "page_publish_rebuild_source::ActiveModel",
], "aggregate validation before manifest/source insertion");
for (const marker of [
  "let source_revision = body.updated_at.to_string();",
  "rebuild_source_provenance_hash(",
  "sanitized_project: Set(row.sanitized_project)",
  "source_revision: Set(row.source_revision)",
  "provenance_hash: Set(row.provenance_hash)",
]) requireText(publishManifest, marker, "publish provenance owner capture");

requireOrder(harness, [
  "page_published_landing_artifact::Entity::delete_many()",
  "page_publish_operation_artifact::Entity::delete_many()",
  "page_static_landing_artifact::Entity::delete_by_id(source.artifact_id)",
  "assert_eq!(deleted.rows_affected, 1);",
  "page_publish_rebuild_source::Entity::find_by_id(source.id)",
  "assert_eq!(retained_after, retained_before);",
], "artifact-row-loss survivability ordering");

if (count(migration, "ForeignKey::create()") !== 1) {
  failures.push("provenance migration must retain exactly one foreign key");
}
for (const marker of [
  'name("fk_page_publish_rebuild_sources_operation")',
  ".to(PagePublishOperations::Table, PagePublishOperations::Id)",
  "PagePublishRebuildSources::ArtifactId",
]) requireText(migration, marker, "provenance migration ownership");
for (const forbidden of [
  "PageStaticLandingArtifacts",
  "fk_page_publish_rebuild_sources_artifact",
  "INSERT INTO page_publish_rebuild_sources",
  "insert_into(",
  "Query::insert",
  "Entity::find",
]) forbidText(migration, forbidden, "provenance migration no-backfill/no-artifact-fk boundary");

for (const marker of [
  "publish-rebuild-provenance-postgres-harness-source-ready",
  "pages_publish_rebuild_provenance_postgres_source_unvalidated",
  "two locales",
  "artifact_set_hash",
  "sanitized_set_hash",
  "artifact row",
  "no backfill",
  "intentionally not run",
]) requireText(continuation, marker, "provenance continuation");

for (const forbidden of [
  "rebuildPageArtifact",
  "activateRebuiltPageArtifact",
  "auditPageArtifacts",
]) forbidText(harness, forbidden, "provenance transport/automatic-repair boundary");

if (failures.length > 0) {
  console.error("[verify-pages-publish-rebuild-provenance-postgres] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-publish-rebuild-provenance-postgres] PASS source_ready=true execution=pending");
