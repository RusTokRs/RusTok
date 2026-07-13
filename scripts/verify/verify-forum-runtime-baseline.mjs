#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";

const args = new Set(process.argv.slice(2));
const allowedArgs = new Set(["--static-only", "--postgres", "--known-defects", "--help"]);
for (const arg of args) {
  if (!allowedArgs.has(arg)) fail(`unknown argument: ${arg}`);
}

const staticOnly = args.has("--static-only");
const postgres = args.has("--postgres");
const knownDefects = args.has("--known-defects");

if (args.has("--help")) {
  console.log(`Usage:
  node scripts/verify/verify-forum-runtime-baseline.mjs [options]

Options:
  --static-only    Validate baseline files and compatibility ignores only.
  --postgres       Run the complete green PostgreSQL forum profile.
  --known-defects  Confirm that FORUM-02..08 has zero active regressions.
  --help           Show this help.`);
  process.exit(0);
}

if (knownDefects && staticOnly) {
  fail("--known-defects cannot be combined with --static-only");
}
if (knownDefects && postgres) {
  fail("--known-defects cannot be combined with --postgres");
}

const files = {
  support: "crates/rustok-forum/tests/support/postgres.rs",
  greenBaseline: "crates/rustok-forum/tests/runtime_regression_baseline.rs",
  knownRegressions: "crates/rustok-forum/tests/known_regressions.rs",
  statusMigration:
    "crates/rustok-forum/src/migrations/m20260712_000004_enforce_forum_status_lifecycle.rs",
  categoryTreeMigration:
    "crates/rustok-forum/src/migrations/m20260712_000005_enforce_forum_category_tree.rs",
  counterLockMigration:
    "crates/rustok-forum/src/migrations/m20260712_000006_serialize_forum_counter_mutations.rs",
  replyPublicationMigration:
    "crates/rustok-forum/src/migrations/m20260713_000007_enforce_forum_reply_publication.rs",
  replyPositionMigration:
    "crates/rustok-forum/src/migrations/m20260713_000008_enforce_forum_reply_positions.rs",
  softDeleteMigration:
    "crates/rustok-forum/src/migrations/m20260713_000009_add_forum_soft_delete_revisions.rs",
  softDeletePostgresUp:
    "crates/rustok-forum/src/migrations/m20260713_000009_add_forum_soft_delete_revisions/postgres_up.rs",
  softDeletePostgresDown:
    "crates/rustok-forum/src/migrations/m20260713_000009_add_forum_soft_delete_revisions/postgres_down.rs",
  softDeleteSqliteUp:
    "crates/rustok-forum/src/migrations/m20260713_000009_add_forum_soft_delete_revisions/sqlite_up.rs",
  softDeleteSqliteDown:
    "crates/rustok-forum/src/migrations/m20260713_000009_add_forum_soft_delete_revisions/sqlite_down.rs",
  softDeleteSqliteSetup:
    "crates/rustok-forum/src/migrations/m20260713_000009_add_forum_soft_delete_revisions/sqlite_setup.rs",
  softDeleteSqliteRevisions:
    "crates/rustok-forum/src/migrations/m20260713_000009_add_forum_soft_delete_revisions/sqlite_revisions.rs",
  softDeleteSqliteDeletes:
    "crates/rustok-forum/src/migrations/m20260713_000009_add_forum_soft_delete_revisions/sqlite_deletes.rs",
  softDeleteSqliteCounters:
    "crates/rustok-forum/src/migrations/m20260713_000009_add_forum_soft_delete_revisions/sqlite_counters.rs",
  revisionService: "crates/rustok-forum/src/services/revision.rs",
  categoryTreePostgres:
    "crates/rustok-forum/tests/category_tree_integrity_postgres.rs",
  counterIntegrityPostgres:
    "crates/rustok-forum/tests/counter_integrity_postgres.rs",
  moderationPostgres:
    "crates/rustok-forum/tests/moderation_semantics_postgres.rs",
  moderationSqlite:
    "crates/rustok-forum/tests/moderation_semantics_sqlite.rs",
  replyPositionPostgres:
    "crates/rustok-forum/tests/reply_position_integrity_postgres.rs",
  replyPositionSqlite:
    "crates/rustok-forum/tests/reply_position_integrity_sqlite.rs",
  softDeletePostgres:
    "crates/rustok-forum/tests/soft_delete_revision_postgres.rs",
  softDeleteSqlite:
    "crates/rustok-forum/tests/soft_delete_revision_sqlite.rs",
};

const resolvedCompatibilityIgnores = new Map([
  ["forum_04_category_cycle_is_rejected", "FORUM-04: category hierarchy must reject cycles"],
  ["forum_05_concurrent_replies_preserve_public_counters", "FORUM-05: concurrent approved replies must preserve topic and category counters"],
  ["forum_06_locked_topic_rejects_reply_creation", "FORUM-06: a locked topic must reject ordinary reply creation"],
  ["forum_06_pending_reply_does_not_change_public_counters", "FORUM-06: pending replies must not mutate public counters"],
  ["forum_06_pending_reply_does_not_emit_public_replied_event", "FORUM-06: pending replies must not publish the public topic-replied event"],
  ["forum_07_concurrent_reply_positions_are_unique_and_contiguous", "FORUM-07: concurrent reply allocation must produce unique contiguous positions"],
  ["forum_07_duplicate_reply_position_is_rejected", "FORUM-07: duplicate reply positions must be rejected by the database"],
]);

const expectedKnownDefects = new Map();

function fail(message) {
  console.error("forum runtime baseline verification failed:");
  console.error(`- ${message}`);
  process.exit(1);
}

function text(path) {
  try {
    return readFileSync(path, "utf8");
  } catch (error) {
    fail(`${path}: ${error.message}`);
  }
}

function requireTokens(source, path, label, tokens) {
  for (const token of tokens) {
    if (!source.includes(token)) {
      fail(`${path}: missing ${label} token ${token}`);
    }
  }
}

function verifyStaticBaseline() {
  for (const path of Object.values(files)) {
    if (!existsSync(path)) fail(`${path}: required baseline file is missing`);
  }

  const support = text(files.support);
  const greenBaseline = text(files.greenBaseline);
  const known = text(files.knownRegressions);
  const statusMigration = text(files.statusMigration);
  const categoryTreeMigration = text(files.categoryTreeMigration);
  const counterLockMigration = text(files.counterLockMigration);
  const replyPublicationMigration = text(files.replyPublicationMigration);
  const replyPositionMigration = text(files.replyPositionMigration);
  const softDeleteMigration = [
    files.softDeleteMigration,
    files.softDeletePostgresUp,
    files.softDeletePostgresDown,
    files.softDeleteSqliteUp,
    files.softDeleteSqliteDown,
    files.softDeleteSqliteSetup,
    files.softDeleteSqliteRevisions,
    files.softDeleteSqliteDeletes,
    files.softDeleteSqliteCounters,
  ]
    .map(text)
    .join("\n");
  const revisionService = text(files.revisionService);

  requireTokens(support, files.support, "PostgreSQL profile", [
    "RUSTOK_FORUM_TEST_DATABASE_URL",
    "PostgresForumTestDb",
    "OutboxModule.migrations()",
    "TaxonomyModule.migrations()",
    "ForumModule.migrations()",
    "DROP SCHEMA IF EXISTS",
  ]);
  requireTokens(greenBaseline, files.greenBaseline, "green baseline", [
    "postgres_forum_tenant_schema_baseline_is_green",
    "REQUIRED_TENANT_CONSTRAINTS",
    "REQUIRED_TENANT_INDEXES",
    "REQUIRED_LIFECYCLE_CONSTRAINTS",
    "REQUIRED_REVISION_TABLES",
  ]);
  requireTokens(statusMigration, files.statusMigration, "lifecycle", [
    "chk_forum_topics_status",
    "chk_forum_replies_status",
  ]);
  requireTokens(categoryTreeMigration, files.categoryTreeMigration, "category tree", [
    "forum_validate_category_parent",
    "forum_categories_tree_guard",
  ]);
  requireTokens(counterLockMigration, files.counterLockMigration, "counter lock", [
    "forum_counter_lock",
    "forum_00_replies_counter_lock",
  ]);
  requireTokens(replyPublicationMigration, files.replyPublicationMigration, "publication", [
    "forum_validate_reply_creation",
    "forum_filter_topic_replied_event",
  ]);
  requireTokens(replyPositionMigration, files.replyPositionMigration, "reply position", [
    "uq_forum_replies_tenant_topic_position",
    "chk_forum_replies_position_positive",
    "forum_lock_reply_counter_mutation",
  ]);
  requireTokens(softDeleteMigration, files.softDeleteMigration, "soft delete", [
    "forum_topic_revisions",
    "forum_reply_revisions",
    "forum_soft_delete_topic",
    "forum_soft_delete_reply",
    "forum_hard_delete_context",
    "idx_forum_topics_tenant_deleted",
    "idx_forum_replies_tenant_topic_deleted",
  ]);
  requireTokens(revisionService, files.revisionService, "revision service", [
    "list_topic_revisions",
    "list_reply_revisions",
    "MAX_REVISION_PAGE_SIZE",
  ]);

  if (known.includes("#[should_panic")) {
    fail(`${files.knownRegressions}: compatibility regressions must not use should_panic`);
  }

  const found = new Map();
  const pattern = /#\[ignore = "([^"]+)"\]\s*async fn ([a-z0-9_]+)/g;
  for (const match of known.matchAll(pattern)) {
    found.set(match[2], match[1]);
  }

  for (const [name, reason] of resolvedCompatibilityIgnores) {
    if (found.get(name) !== reason) {
      fail(`${files.knownRegressions}: compatibility ignore ${name} is missing or changed`);
    }
  }

  const tracked = new Map(
    [...found].filter(([name]) => !resolvedCompatibilityIgnores.has(name)),
  );
  if (tracked.size !== expectedKnownDefects.size) {
    fail(`${files.knownRegressions}: expected zero active regressions, found ${tracked.size}`);
  }

  console.log(
    `forum runtime baseline static verification passed (${tracked.size} tracked known defects)`,
  );
}

function run(label, command, commandArgs) {
  console.log(`\n==> ${label}`);
  const executable =
    process.platform === "win32" && command === "cargo" ? "cargo.exe" : command;
  const result = spawnSync(executable, commandArgs, {
    cwd: process.cwd(),
    env: process.env,
    stdio: "inherit",
  });
  if (result.error) fail(`${label}: ${result.error.message}`);
  if (result.status !== 0) fail(`${label}: command exited with status ${result.status}`);
}

function requirePostgresUrl(mode) {
  const value =
    process.env.RUSTOK_FORUM_TEST_DATABASE_URL ?? process.env.DATABASE_URL ?? "";
  if (!value.startsWith("postgres://") && !value.startsWith("postgresql://")) {
    fail(
      `${mode} requires RUSTOK_FORUM_TEST_DATABASE_URL (or DATABASE_URL) with a PostgreSQL URL`,
    );
  }
}

verifyStaticBaseline();

if (staticOnly) process.exit(0);

if (knownDefects) {
  console.log("FORUM-02..08 has zero active runtime regressions.");
  process.exit(0);
}

run("Rust formatting", "cargo", ["fmt", "--all", "--", "--check"]);
run("forum library tests", "cargo", ["test", "-p", "rustok-forum", "--lib"]);
run("green runtime regression baseline", "cargo", [
  "test", "-p", "rustok-forum", "--test", "runtime_regression_baseline",
]);
run("known regression compile/list check", "cargo", [
  "test", "-p", "rustok-forum", "--test", "known_regressions", "--", "--list",
]);

for (const testName of [
  "tenant_child_integrity_sqlite",
  "tenant_relation_integrity_sqlite",
  "status_lifecycle_sqlite",
  "category_atomicity_sqlite",
  "category_tree_integrity_sqlite",
  "moderation_semantics_sqlite",
  "reply_position_integrity_sqlite",
  "soft_delete_revision_sqlite",
]) {
  run(`SQLite ${testName}`, "cargo", [
    "test", "-p", "rustok-forum", "--test", testName,
  ]);
}

for (const testName of [
  "category_tree_integrity_postgres",
  "counter_integrity_postgres",
  "moderation_semantics_postgres",
  "reply_position_integrity_postgres",
  "soft_delete_revision_postgres",
]) {
  run(`PostgreSQL ${testName}`, "cargo", [
    "test", "-p", "rustok-forum", "--test", testName,
  ]);
}

run("content orchestration compatibility", "cargo", [
  "test", "-p", "rustok-content-orchestration",
]);

if (postgres) {
  requirePostgresUrl("--postgres");
  for (const testName of [
    "tenant_integrity",
    "tenant_child_integrity_postgres",
    "tenant_relation_integrity_postgres",
    "runtime_regression_baseline",
    "counter_integrity_postgres",
    "moderation_semantics_postgres",
    "reply_position_integrity_postgres",
    "soft_delete_revision_postgres",
    "known_regressions",
  ]) {
    run(`PostgreSQL ${testName}`, "cargo", [
      "test",
      "-p",
      "rustok-forum",
      "--test",
      testName,
      "--",
      "--nocapture",
      "--test-threads=1",
    ]);
  }
}

console.log("\nforum runtime baseline verification passed");
