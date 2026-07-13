#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";

const args = new Set(process.argv.slice(2));
const allowedArgs = new Set(["--static-only", "--postgres", "--known-defects", "--help"]);
for (const arg of args) {
  if (!allowedArgs.has(arg)) {
    fail(`unknown argument: ${arg}`);
  }
}

const staticOnly = args.has("--static-only");
const postgres = args.has("--postgres");
const knownDefects = args.has("--known-defects");

if (args.has("--help")) {
  console.log(`Usage:
  node scripts/verify/verify-forum-runtime-baseline.mjs [options]

Options:
  --static-only    Validate baseline files and the ignored-test inventory only.
  --postgres       Run the green PostgreSQL tenant regression profile.
  --known-defects  Run ignored FORUM-07 reproductions explicitly.
                    This diagnostic mode is expected to fail until defects are fixed.
  --help           Show this help.`);
  process.exit(0);
}

if (knownDefects && staticOnly) {
  fail("--known-defects cannot be combined with --static-only");
}
if (knownDefects && postgres) {
  fail("--known-defects already selects the PostgreSQL diagnostic profile; omit --postgres");
}

const files = {
  support: "crates/rustok-forum/tests/support/postgres.rs",
  greenBaseline: "crates/rustok-forum/tests/runtime_regression_baseline.rs",
  knownRegressions: "crates/rustok-forum/tests/known_regressions.rs",
  statusMigration:
    "crates/rustok-forum/src/migrations/m20260712_000004_enforce_forum_status_lifecycle.rs",
  categoryTreeMigration:
    "crates/rustok-forum/src/migrations/m20260712_000005_enforce_forum_category_tree.rs",
  categoryTreePostgres:
    "crates/rustok-forum/tests/category_tree_integrity_postgres.rs",
  counterLockMigration:
    "crates/rustok-forum/src/migrations/m20260712_000006_serialize_forum_counter_mutations.rs",
  counterIntegrityPostgres:
    "crates/rustok-forum/tests/counter_integrity_postgres.rs",
  replyPublicationMigration:
    "crates/rustok-forum/src/migrations/m20260713_000007_enforce_forum_reply_publication.rs",
  moderationService: "crates/rustok-forum/src/services/moderation.rs",
  moderationPostgres:
    "crates/rustok-forum/tests/moderation_semantics_postgres.rs",
  moderationSqlite:
    "crates/rustok-forum/tests/moderation_semantics_sqlite.rs",
};

const resolvedCompatibilityIgnores = new Map([
  ["forum_04_category_cycle_is_rejected", "FORUM-04: category hierarchy must reject cycles"],
  ["forum_05_concurrent_replies_preserve_public_counters", "FORUM-05: concurrent approved replies must preserve topic and category counters"],
  ["forum_06_locked_topic_rejects_reply_creation", "FORUM-06: a locked topic must reject ordinary reply creation"],
  ["forum_06_pending_reply_does_not_change_public_counters", "FORUM-06: pending replies must not mutate public counters"],
  ["forum_06_pending_reply_does_not_emit_public_replied_event", "FORUM-06: pending replies must not publish the public topic-replied event"],
]);

const expectedKnownDefects = new Map([
  ["forum_07_concurrent_reply_positions_are_unique_and_contiguous", "FORUM-07: concurrent reply allocation must produce unique contiguous positions"],
  ["forum_07_duplicate_reply_position_is_rejected", "FORUM-07: duplicate reply positions must be rejected by the database"],
]);

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
  const counterIntegrityPostgres = text(files.counterIntegrityPostgres);
  const replyPublicationMigration = text(files.replyPublicationMigration);
  const moderationService = text(files.moderationService);
  const moderationPostgres = text(files.moderationPostgres);
  const moderationSqlite = text(files.moderationSqlite);

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
  ]);
  requireTokens(statusMigration, files.statusMigration, "lifecycle", [
    "chk_forum_topics_status",
    "chk_forum_replies_status",
    "forum_topics_status_insert",
    "forum_replies_status_insert",
  ]);
  requireTokens(categoryTreeMigration, files.categoryTreeMigration, "category-tree", [
    "forum_validate_category_parent",
    "forum_categories_tree_guard",
    "forum_categories_tree_insert",
    "forum_categories_tree_update",
  ]);
  requireTokens(counterLockMigration, files.counterLockMigration, "counter-lock", [
    "forum_counter_lock",
    "forum_00_topics_counter_lock",
    "forum_00_replies_counter_lock",
    "forum_00_solutions_counter_lock",
  ]);
  requireTokens(counterIntegrityPostgres, files.counterIntegrityPostgres, "counter regression", [
    "concurrent_replies_preserve_atomic_counters",
    "forum_user_stats",
    "expected=8",
  ]);
  requireTokens(replyPublicationMigration, files.replyPublicationMigration, "publication", [
    "forum_validate_reply_creation",
    "forum_enforce_topic_public_reply_count",
    "forum_enforce_category_public_reply_count",
    "forum_enforce_user_public_reply_count",
    "forum_filter_topic_replied_event",
    "forum_replies_locked_topic_insert",
    "forum_topic_replied_visibility_insert",
  ]);
  requireTokens(moderationService, files.moderationService, "moderation service", [
    "became_public",
    "stopped_being_public",
    "DomainEvent::ForumTopicReplied",
    "CategoryService::adjust_counters_in_tx",
  ]);
  requireTokens(moderationPostgres, files.moderationPostgres, "PostgreSQL moderation regression", [
    "postgres_enforces_locked_and_moderated_reply_semantics",
    "assert_public_state",
    "expected_replied_events",
  ]);
  requireTokens(moderationSqlite, files.moderationSqlite, "SQLite moderation regression", [
    "sqlite_enforces_locked_and_moderated_reply_semantics",
    "assert_public_state",
    "expected_replied_events",
  ]);

  for (const token of [
    "forum_validate_category_parent",
    "forum_categories_tree_guard",
    "forum_categories_tree_insert",
    "forum_categories_tree_update",
  ]) {
    if (!categoryTreeMigration.includes(token)) {
      fail(`${files.categoryTreeMigration}: missing category-tree token ${token}`);
    }
  }

  if (known.includes("#[should_panic")) {
    fail(`${files.knownRegressions}: known defects must be real ignored tests, not should_panic placeholders`);
  }

  const found = new Map();
  const pattern = /#\[ignore = "([^"]+)"\]\s*async fn ([a-z0-9_]+)/g;
  for (const match of known.matchAll(pattern)) {
    found.set(match[2], match[1]);
  }

  for (const [name, reason] of resolvedCompatibilityIgnores) {
    if (found.get(name) !== reason) {
      fail(`${files.knownRegressions}: resolved compatibility ignore ${name} is missing or changed`);
    }
  }

  const tracked = new Map(
    [...found].filter(([name]) => !resolvedCompatibilityIgnores.has(name)),
  );

  if (tracked.size !== expectedKnownDefects.size) {
    fail(`${files.knownRegressions}: expected ${expectedKnownDefects.size} ignored regressions, found ${tracked.size}`);
  }

  for (const [name, reason] of expectedKnownDefects) {
    if (!tracked.has(name)) {
      fail(`${files.knownRegressions}: missing ignored regression ${name}`);
    }
    if (tracked.get(name) !== reason) {
      fail(`${files.knownRegressions}: ${name} ignore reason changed; expected "${reason}", found "${tracked.get(name)}"`);
    }
  }

  const unexpected = [...tracked.keys()].filter((name) => !expectedKnownDefects.has(name));
  if (unexpected.length > 0) {
    fail(`${files.knownRegressions}: unexpected ignored regressions: ${unexpected.join(", ")}`);
  }

  console.log(`forum runtime baseline static verification passed (${tracked.size} tracked known defects)`);
}

function run(label, command, commandArgs) {
  console.log(`\n==> ${label}`);
  const executable = process.platform === "win32" && command === "cargo" ? "cargo.exe" : command;
  const result = spawnSync(executable, commandArgs, {
    cwd: process.cwd(),
    env: process.env,
    stdio: "inherit",
  });
  if (result.error) {
    fail(`${label}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    fail(`${label}: command exited with status ${result.status}`);
  }
}

function requirePostgresUrl(mode) {
  const value = process.env.RUSTOK_FORUM_TEST_DATABASE_URL ?? process.env.DATABASE_URL ?? "";
  if (!value.startsWith("postgres://") && !value.startsWith("postgresql://")) {
    fail(`${mode} requires RUSTOK_FORUM_TEST_DATABASE_URL (or DATABASE_URL) with a PostgreSQL URL`);
  }
}

verifyStaticBaseline();

if (staticOnly) process.exit(0);

if (knownDefects) {
  requirePostgresUrl("--known-defects");
  run("ignored FORUM-07 reproductions", "cargo", [
    "test",
    "-p",
    "rustok-forum",
    "--test",
    "known_regressions",
    "--",
    "--ignored",
    "--nocapture",
    "--test-threads=1",
  ]);
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
]) {
  run(`SQLite ${testName}`, "cargo", [
    "test", "-p", "rustok-forum", "--test", testName,
  ]);
}

for (const testName of [
  "category_tree_integrity_postgres",
  "counter_integrity_postgres",
  "moderation_semantics_postgres",
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
