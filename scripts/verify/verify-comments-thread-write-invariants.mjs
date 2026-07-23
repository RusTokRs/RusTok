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
const servicesPath = "crates/rustok-comments/src/services.rs";
const migrationPath =
  "crates/rustok-comments/src/migrations/m20260723_000008_repair_comment_thread_counters.rs";
const migrationRegistryPath = "crates/rustok-comments/src/migrations/mod.rs";
const testPath = "crates/rustok-comments/tests/thread_write_invariants.rs";
const evidencePath =
  "crates/rustok-comments/contracts/evidence/comments-thread-write-invariants.json";
const planPath = "crates/rustok-comments/docs/implementation-plan.md";

const comment = read(commentPath);
const thread = read(threadPath);
const services = read(servicesPath);
const migration = read(migrationPath);
const migrationRegistry = read(migrationRegistryPath);
const test = read(testPath);
const plan = read(planPath);
let evidence = null;
try {
  evidence = JSON.parse(read(evidencePath));
} catch (error) {
  failures.push(`${evidencePath}: invalid JSON: ${error.message}`);
}

for (const marker of [
  "impl ActiveModelBehavior for ActiveModel",
  "async fn before_save",
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
  "async fn before_save",
  "if insert",
  "comment thread {thread_id} is missing while refreshing counters",
  "update_many()",
  "Column::TenantId.eq(tenant_id)",
  "DeletedAt.is_null()",
  ".count(db)",
  "self.comment_count = Set(count)",
]) {
  requireMarker(thread, marker, threadPath);
}

const counterHelperStart = services.indexOf("async fn update_thread_counters_in_tx");
if (counterHelperStart === -1) {
  failures.push(`${servicesPath}: missing update_thread_counters_in_tx`);
} else {
  const counterHelper = services.slice(counterHelperStart, services.indexOf("\n    fn ", counterHelperStart));
  requireMarker(counterHelper, "active.update(txn).await?", `${servicesPath}: counter helper`);
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
  requireMarker(migration, marker, migrationPath);
}

for (const marker of [
  "mod m20260723_000008_repair_comment_thread_counters;",
  "Box::new(m20260723_000008_repair_comment_thread_counters::Migration)",
]) {
  requireMarker(migrationRegistry, marker, migrationRegistryPath);
}

for (const marker of [
  "active_model_hooks_override_stale_positions_and_counts",
  "unique_position_index_rejects_active_model_bypass",
  "stale_thread.comment_count = Set(999)",
  "assert_eq!(first.position, 1)",
  "assert_eq!(second.position, 2)",
  "assert_eq!(repaired.comment_count, 1)",
  "comment::Entity::insert",
]) {
  requireMarker(test, marker, testPath);
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
    counter_owner: threadPath,
    repair_migration: migrationPath,
    migration_registry: migrationRegistryPath,
    executable_test: testPath,
  })) {
    if (contract[key] !== expected) failures.push(`${evidencePath}: ${key} path drift`);
  }
  const cases = new Set((evidence.cases ?? []).map((entry) => entry.name));
  for (const requiredCase of [
    "serialized_position_allocation",
    "exact_active_comment_count",
    "historical_counter_repair",
    "historical_position_repair",
    "bulk_bypass_rejection",
  ]) {
    if (!cases.has(requiredCase)) failures.push(`${evidencePath}: missing case ${requiredCase}`);
  }
}

for (const marker of [
  "comments-thread-write-invariants.json",
  "thread_write_invariants",
  "ActiveModelBehavior",
  "UNIQUE(thread_id, position)",
]) {
  requireMarker(plan, marker, planPath);
}

if (failures.length > 0) {
  console.error("comments thread write invariant verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("comments thread write invariant verification passed");
