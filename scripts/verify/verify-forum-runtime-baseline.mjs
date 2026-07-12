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
  --known-defects  Run ignored FORUM-03..07 reproductions explicitly.
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
};

const expectedKnownDefects = new Map([
  ["forum_03_category_create_rolls_back_when_translation_insert_fails", "FORUM-03: category and initial translation creation must be atomic"],
  ["forum_04_category_cycle_is_rejected", "FORUM-04: category hierarchy must reject cycles"],
  ["forum_05_concurrent_replies_preserve_public_counters", "FORUM-05: concurrent approved replies must preserve topic and category counters"],
  ["forum_06_locked_topic_rejects_reply_creation", "FORUM-06: a locked topic must reject ordinary reply creation"],
  ["forum_06_pending_reply_does_not_change_public_counters", "FORUM-06: pending replies must not mutate public counters"],
  ["forum_06_pending_reply_does_not_emit_public_replied_event", "FORUM-06: pending replies must not publish the public topic-replied event"],
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

function verifyStaticBaseline() {
  for (const path of Object.values(files)) {
    if (!existsSync(path)) fail(`${path}: required baseline file is missing`);
  }

  const support = text(files.support);
  const greenBaseline = text(files.greenBaseline);
  const known = text(files.knownRegressions);
  const statusMigration = text(files.statusMigration);

  for (const token of [
    "RUSTOK_FORUM_TEST_DATABASE_URL",
    "PostgresForumTestDb",
    "OutboxModule.migrations()",
    "TaxonomyModule.migrations()",
    "ForumModule.migrations()",
    "DROP SCHEMA IF EXISTS",
  ]) {
    if (!support.includes(token)) {
      fail(`${files.support}: missing PostgreSQL profile token ${token}`);
    }
  }

  for (const token of [
    "postgres_forum_tenant_schema_baseline_is_green",
    "REQUIRED_TENANT_CONSTRAINTS",
    "REQUIRED_TENANT_INDEXES",
    "REQUIRED_LIFECYCLE_CONSTRAINTS",
  ]) {
    if (!greenBaseline.includes(token)) {
      fail(`${files.greenBaseline}: missing green baseline token ${token}`);
    }
  }

  for (const token of [
    "chk_forum_topics_status",
    "chk_forum_replies_status",
    "forum_topics_status_insert",
    "forum_replies_status_insert",
  ]) {
    if (!statusMigration.includes(token)) {
      fail(`${files.statusMigration}: missing lifecycle token ${token}`);
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

  if (found.size !== expectedKnownDefects.size) {
    fail(`${files.knownRegressions}: expected ${expectedKnownDefects.size} ignored regressions, found ${found.size}`);
  }

  for (const [name, reason] of expectedKnownDefects) {
    if (!found.has(name)) {
      fail(`${files.knownRegressions}: missing ignored regression ${name}`);
    }
    if (found.get(name) !== reason) {
      fail(`${files.knownRegressions}: ${name} ignore reason changed; expected "${reason}", found "${found.get(name)}"`);
    }
  }

  const unexpected = [...found.keys()].filter((name) => !expectedKnownDefects.has(name));
  if (unexpected.length > 0) {
    fail(`${files.knownRegressions}: unexpected ignored regressions: ${unexpected.join(", ")}`);
  }

  console.log(`forum runtime baseline static verification passed (${found.size} tracked known defects)`);
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
  run("ignored FORUM-03..07 reproductions", "cargo", [
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
  "test",
  "-p",
  "rustok-forum",
  "--test",
  "runtime_regression_baseline",
]);
run("known regression compile/list check", "cargo", [
  "test",
  "-p",
  "rustok-forum",
  "--test",
  "known_regressions",
  "--",
  "--list",
]);
run("SQLite tenant child regression", "cargo", [
  "test",
  "-p",
  "rustok-forum",
  "--test",
  "tenant_child_integrity_sqlite",
]);
run("SQLite tenant relation regression", "cargo", [
  "test",
  "-p",
  "rustok-forum",
  "--test",
  "tenant_relation_integrity_sqlite",
]);
run("SQLite lifecycle status regression", "cargo", [
  "test",
  "-p",
  "rustok-forum",
  "--test",
  "status_lifecycle_sqlite",
]);
run("content orchestration compatibility", "cargo", [
  "test",
  "-p",
  "rustok-content-orchestration",
]);

if (postgres) {
  requirePostgresUrl("--postgres");
  for (const testName of [
    "tenant_integrity",
    "tenant_child_integrity_postgres",
    "tenant_relation_integrity_postgres",
    "runtime_regression_baseline",
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
