#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const files = {
  evidence: "crates/rustok-blog/contracts/evidence/blog-category-translation-postgres-source.json",
  harness: "crates/rustok-blog/tests/category_translation_target_postgres_test.rs",
  migration: "crates/rustok-blog/src/migrations/m20260803_000016_add_blog_category_translation_target_support.rs",
  migrations: "crates/rustok-blog/src/migrations/mod.rs",
  owner: "crates/rustok-blog/src/services/category.rs",
  provider: "crates/rustok-blog/src/translation_target.rs",
  changeWriter: "crates/rustok-blog/src/translation_evidence.rs",
  coreId: "crates/rustok-core/src/id.rs",
  sqlitePilot: "crates/rustok-blog/src/translation_target_tests.rs",
  rootPlan: "crates/rustok-blog/docs/implementation-plan.md",
  plan: "crates/rustok-blog/docs/implementation-plan-slice-98.md",
  translationPlan: "docs/modules/translation-implementation-plan.md",
};

const failures = [];
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

if (failures.length) {
  console.error("[verify-blog-category-translation-postgres-source] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const evidence = JSON.parse(read(files.evidence));
const harness = read(files.harness);
const migration = read(files.migration);
const migrations = read(files.migrations);
const owner = read(files.owner);
const provider = read(files.provider);
const changeWriter = read(files.changeWriter);
const coreId = read(files.coreId);
const sqlitePilot = read(files.sqlitePilot);
const rootPlan = read(files.rootPlan);
const plan = read(files.plan);
const translationPlan = read(files.translationPlan);

if (
  evidence.schema_version !== 1 ||
  evidence.status !== "blog_category_translation_postgres_source_unvalidated" ||
  evidence.owner !== "rustok-blog" ||
  evidence.provider !== "blog/category"
) {
  failures.push("evidence identity/status/owner/provider drifted");
}

for (const key of [
  "postgres_environment_gated_harness_added",
  "isolated_postgres_schema_per_scenario",
  "search_path_excludes_public",
  "real_outbox_migrations_used",
  "real_taxonomy_migrations_used",
  "real_blog_migrations_used",
  "translation_target_migration_up_down_up_covered",
  "reapplied_revision_columns_exercised_through_category_service",
  "reapplied_change_journal_exercised_through_provider",
  "concurrent_apply_uses_independent_database_connections",
  "concurrent_apply_uses_same_source_snapshot_and_expected_revisions",
  "concurrent_apply_uses_distinct_idempotency_keys",
  "concurrent_apply_requires_exactly_one_success",
  "concurrent_apply_requires_closed_conflict_loser",
  "concurrent_apply_requires_one_target_revision",
  "concurrent_apply_requires_one_winning_change_fact",
  "concurrent_apply_requires_one_reindex_outbox_event",
  "cursor_recovery_starts_from_source_create_cursor",
  "cursor_recovery_reconstructs_provider_before_apply",
  "cursor_recovery_resumes_exactly_one_apply_change",
  "cursor_recovery_reconstructs_owner_before_delete",
  "cursor_recovery_reconstructs_provider_after_delete",
  "cursor_recovery_resumes_deleted_lifecycle_change",
  "cursor_recovery_drains_after_latest_cursor",
  "progress_cursor_matches_latest_deleted_change",
  "ulid_cursor_generation_is_separated_for_deterministic_recovery_fixture"
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`source_contract.${key} must be true`);
  }
}
for (const key of [
  "concurrent_cursor_commit_order_claimed",
  "production_blog_behavior_changed",
  "production_translation_contract_changed",
  "database_schema_changed",
  "ffa_promoted",
  "fba_promoted",
  "postgres_execution_observed"
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`source_contract.${key} must remain false`);
  }
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence.execution must remain empty before maintainer execution");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}

for (const marker of [
  "RUSTOK_BLOG_TRANSLATION_TEST_DATABASE_URL",
  'SET search_path TO "{schema_name}"',
  "for migration in OutboxModule.migrations()",
  "for migration in TaxonomyModule.migrations()",
  "for migration in BlogModule.migrations()",
  "category_translation_target_migration_supports_postgres_up_down_up",
  "translation_target_migration.up(&manager).await?",
  "translation_target_migration.down(&manager).await?",
  "concurrent_same_revision_translation_applies_commit_once",
  "Barrier::new(candidates.len())",
  "exactly one same-revision apply must commit",
  "PortErrorKind::Conflict",
  "source create plus one winning apply only",
  "only the winning apply may publish reindex",
  "change_cursor_resumes_after_provider_reconstruction_and_delete",
  "after: Some(first_cursor.clone())",
  "after: Some(second_cursor.clone())",
  "TranslationResourceLifecycle::Deleted",
  "assert!(drained.changes.is_empty())",
  "assert_eq!(progress.owner_change_cursor, Some(deleted_cursor))",
  "tokio::time::sleep(Duration::from_millis(2))",
  "does not claim a concurrent commit-order guarantee"
]) need(harness, marker, "PostgreSQL harness");
for (const marker of [
  "CREATE TABLE blog_translation_changes",
  "INSERT INTO blog_translation_changes",
  "UPDATE blog_translation_changes",
  "DELETE FROM blog_translation_changes",
  "SET search_path TO public",
  "SET search_path TO \"public\""
]) forbid(harness, marker, "PostgreSQL harness");

for (const marker of [
  "m20260803_000016_add_blog_category_translation_target_support",
  "BlogTranslationChanges::Table",
  "BlogCategories::Revision",
  "BlogCategoryTranslations::Revision",
  "idx_blog_translation_changes_tenant_id"
]) need(migration + migrations, marker, "Blog translation migration");

for (const marker of [
  "apply_exact_translation_in_tx",
  "blog category resource revision does not match the translation proposal",
  "source locale revision does not match the translation proposal",
  "target locale revision does not match the translation proposal",
  ".filter(blog_category_translation::Column::Revision.eq(target.revision))",
  ".filter(blog_category::Column::Revision.eq(category.revision))",
  "record_translation_change_in_tx",
  "publish_blog_reindex_in_tx"
]) need(owner, marker, "Category owner CAS");

for (const marker of [
  "idempotency::admit",
  "apply_exact_translation_in_tx",
  "async fn read_changes",
  "order_by_asc(TranslationChangeColumn::Id)",
  "filter(TranslationChangeColumn::Id.gt(after))",
  "PROGRESS_STABILITY_ATTEMPTS",
  "owner_change_cursor"
]) need(provider, marker, "Blog Translation provider");

need(changeWriter, "id: Set(generate_id())", "Blog change writer");
need(coreId, "Uuid::from_bytes(Ulid::r#gen().to_bytes())", "ULID-backed core id");
for (const marker of [
  "translation_target_schema_supports_up_down_up",
  "translation_target_applies_replays_and_tracks_an_exact_category_locale"
]) need(sqlitePilot, marker, "SQLite pilot baseline");

need(
  rootPlan,
  "Retained PostgreSQL migration, concurrent CAS, and change-cursor recovery evidence are still required before production inventory enablement",
  "Blog root plan open pilot result",
);
for (const marker of [
  "category_translation_postgres_evidence_source_ready_maintainer_execution_pending",
  "PostgreSQL migration up/down/up",
  "Concurrent same-revision apply",
  "Change-cursor recovery after reconstruction",
  "does **not** claim that the current cursor contract proves arbitrary concurrent transaction commit ordering",
  "No tests, Cargo commands, Node verifiers"
]) need(plan, marker, "slice 98 plan");
for (const marker of [
  "Blog's",
  "`blog/category` provider",
  "records an append-only Blog change cursor"
]) need(translationPlan, marker, "Translation owner plan");

if (failures.length) {
  console.error("[verify-blog-category-translation-postgres-source] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "[verify-blog-category-translation-postgres-source] PASS source_ready=true execution=not_run provider=blog/category",
);
