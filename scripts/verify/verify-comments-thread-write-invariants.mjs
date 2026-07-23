#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function read(relativePath) {
  const target = repoPath(relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(target, "utf8");
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

const commentPath = "crates/rustok-comments/src/entities/comment.rs";
const threadPath = "crates/rustok-comments/src/entities/comment_thread.rs";
const identityEntityPath =
  "crates/rustok-comments/src/entities/comment_thread_identity_lock.rs";
const servicesPath = "crates/rustok-comments/src/services.rs";
const counterMigrationPath =
  "crates/rustok-comments/src/migrations/m20260723_000008_repair_comment_thread_counters.rs";
const identityMigrationPath =
  "crates/rustok-comments/src/migrations/m20260723_000009_add_comment_thread_identity_locks.rs";
const migrationRegistryPath = "crates/rustok-comments/src/migrations/mod.rs";
const writeTestPath = "crates/rustok-comments/tests/thread_write_invariants.rs";
const firstThreadTestPath =
  "crates/rustok-comments/tests/thread_creation_concurrency.rs";
const evidencePath =
  "crates/rustok-comments/contracts/evidence/comments-thread-write-invariants.json";
const planPath = "crates/rustok-comments/docs/implementation-plan.md";

const comment = read(commentPath);
const thread = read(threadPath);
const identityEntity = read(identityEntityPath);
const services = read(servicesPath);
const counterMigration = read(counterMigrationPath);
const identityMigration = read(identityMigrationPath);
const migrationRegistry = read(migrationRegistryPath);
const writeTest = read(writeTestPath);
const firstThreadTest = read(firstThreadTestPath);
const plan = read(planPath);
let evidence = null;
try {
  evidence = JSON.parse(read(evidencePath));
} catch (error) {
  failures.push(`${evidencePath}: invalid JSON: ${error.message}`);
}

for (const marker of [
  "impl ActiveModelBehavior for ActiveModel",
  "if !insert",
  "comment thread {thread_id} is missing while allocating a position",
  "update_many()",
  "Column::TenantId.eq(tenant_id)",
  "order_by_desc(Column::Position)",
  "checked_add(1)",
  "self.position = Set(next_position)",
]) {
  requireMarker(comment, marker, commentPath);
}

for (const marker of [
  "impl ActiveModelBehavior for ActiveModel",
  "serialize_thread_identity(db, &self).await?",
  "matches!(&self.comment_count, ActiveValue::Set(_))",
  "comment thread {thread_id} is missing while refreshing counters",
  "update_many()",
  "Column::TenantId.eq(tenant_id)",
  "DeletedAt.is_null()",
  ".count(db)",
  "self.comment_count = Set(count)",
  "OnConflict::columns",
  "identity_lock::Entity::update_many()",
  "comment thread identity {target_type}:{target_id} already belongs to",
]) {
  requireMarker(thread, marker, threadPath);
}

for (const marker of [
  '#[sea_orm(table_name = "comment_thread_identity_locks")]',
  "pub tenant_id: Uuid",
  "pub target_type: String",
  "pub target_id: Uuid",
  "impl ActiveModelBehavior for ActiveModel",
]) {
  requireMarker(identityEntity, marker, identityEntityPath);
}

const counterHelperStart = services.indexOf("async fn update_thread_counters_in_tx");
if (counterHelperStart === -1) {
  failures.push(`${servicesPath}: missing update_thread_counters_in_tx`);
} else {
  const helperEnd = services.indexOf("\n    fn ", counterHelperStart);
  const counterHelper = services.slice(
    counterHelperStart,
    helperEnd === -1 ? services.length : helperEnd,
  );
  requireMarker(counterHelper, "active.update(txn).await?", `${servicesPath}: counter helper`);
}
for (const marker of [
  "find_or_create_thread_in_tx",
  "match thread.insert(txn).await",
  "Err(_) => comment_thread::Entity::find()",
]) {
  requireMarker(services, marker, servicesPath);
}

for (const marker of [
  "DatabaseBackend::Postgres",
  "DatabaseBackend::Sqlite",
  "UPDATE comment_threads",
  "COUNT(comment_row.id)::INTEGER",
  "ROW_NUMBER() OVER",
  "PARTITION BY thread_id",
  "ORDER BY position ASC, created_at ASC, id ASC",
  ".unique()",
  'name("idx_comments_thread_position")',
]) {
  requireMarker(counterMigration, marker, counterMigrationPath);
}

for (const marker of [
  "CommentThreadIdentityLocks::Table",
  "CommentThreadIdentityLocks::TenantId",
  "CommentThreadIdentityLocks::TargetType",
  "CommentThreadIdentityLocks::TargetId",
  'name("idx_comment_thread_identity_locks_identity")',
  ".unique()",
]) {
  requireMarker(identityMigration, marker, identityMigrationPath);
}

for (const marker of [
  "mod m20260723_000008_repair_comment_thread_counters;",
  "Box::new(m20260723_000008_repair_comment_thread_counters::Migration)",
  "mod m20260723_000009_add_comment_thread_identity_locks;",
  "Box::new(m20260723_000009_add_comment_thread_identity_locks::Migration)",
]) {
  requireMarker(migrationRegistry, marker, migrationRegistryPath);
}

for (const marker of [
  "active_model_hooks_override_stale_positions_and_counts",
  "status_only_thread_update_preserves_comment_count",
  "unique_position_index_rejects_active_model_bypass",
  "postgres_concurrent_creates_and_delete_preserve_thread_invariants",
  "RUSTOK_COMMENTS_TEST_DATABASE_URL",
  "tokio::join!",
  "max_connections(1)",
  'SET search_path TO "{schema_name}", public',
  "assert_eq!(positions, vec![1, 2, 3])",
  "assert_eq!(thread.comment_count, active_count as i32)",
]) {
  requireMarker(writeTest, marker, writeTestPath);
}

for (const marker of [
  "postgres_concurrent_first_comments_share_one_thread",
  "CommentsService::new(test_db.db_a.clone())",
  "CommentsService::new(test_db.db_b.clone())",
  "tokio::join!",
  "assert_eq!(first.thread_id, second.thread_id)",
  "assert_eq!(positions, HashSet::from([1, 2]))",
  "assert_eq!(threads.len(), 1)",
  "assert_eq!(threads[0].comment_count, 2)",
  "RUSTOK_COMMENTS_TEST_DATABASE_URL",
]) {
  requireMarker(firstThreadTest, marker, firstThreadTestPath);
}

if (evidence) {
  if (evidence.schema_version !== 1) failures.push(`${evidencePath}: schema_version drift`);
  if (
    evidence.module !== "comments" ||
    evidence.surface !== "thread_write_invariants" ||
    evidence.owner !== "rustok-comments"
  ) {
    failures.push(`${evidencePath}: identity drift`);
  }
  if (evidence.status !== "executable_no_run") failures.push(`${evidencePath}: status drift`);
  if (evidence.compile_policy !== "not_run_by_request") {
    failures.push(`${evidencePath}: compile policy drift`);
  }
  const contract = evidence.production_contract ?? {};
  for (const [key, expected] of Object.entries({
    position_owner: commentPath,
    counter_and_identity_owner: threadPath,
    identity_lock_entity: identityEntityPath,
    counter_repair_migration: counterMigrationPath,
    identity_lock_migration: identityMigrationPath,
    migration_registry: migrationRegistryPath,
    write_invariant_test: writeTestPath,
    first_thread_test: firstThreadTestPath,
    postgres_environment: "RUSTOK_COMMENTS_TEST_DATABASE_URL",
  })) {
    if (contract[key] !== expected) failures.push(`${evidencePath}: ${key} drift`);
  }
  const cases = new Set((evidence.cases ?? []).map((entry) => entry.name));
  for (const requiredCase of [
    "serialized_position_allocation",
    "exact_active_comment_count",
    "status_only_update_preserves_count",
    "historical_counter_repair",
    "historical_position_repair",
    "bulk_bypass_rejection",
    "postgres_concurrent_create_delete",
    "postgres_concurrent_first_thread_creation",
  ]) {
    if (!cases.has(requiredCase)) failures.push(`${evidencePath}: missing case ${requiredCase}`);
  }
}

for (const marker of [
  "comments-thread-write-invariants.json",
  "thread_write_invariants",
  "ActiveModelBehavior",
  "UNIQUE(thread_id, position)",
  "RUSTOK_COMMENTS_TEST_DATABASE_URL",
  "concurrent PostgreSQL",
  "identity-lock",
  "thread_creation_concurrency",
]) {
  requireMarker(plan, marker, planPath);
}

if (failures.length > 0) {
  console.error("comments thread write invariant verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("comments thread write invariant verification passed");
