#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";

const args = new Set(process.argv.slice(2));
const staticOnly = args.has("--static-only");

if (args.has("--help")) {
  console.log(`Usage:
  node scripts/verify/verify-forum-read-model.mjs [--static-only]

Checks the bounded keyset-cursor contract for forum categories, topics, and replies.`);
  process.exit(0);
}

for (const arg of args) {
  if (!["--static-only", "--help"].includes(arg)) {
    fail(`unknown argument: ${arg}`);
  }
}

const paths = {
  dto: "crates/rustok-forum/src/dto/read_model.rs",
  service: "crates/rustok-forum/src/services/read_model.rs",
  compatibility: "crates/rustok-forum/src/services/bounded_compat.rs",
  categoryOwner: "crates/rustok-forum/src/services/category_owner.rs",
  servicesRegistry: "crates/rustok-forum/src/services/mod.rs",
  topicDto: "crates/rustok-forum/src/dto/topic.rs",
  replyDto: "crates/rustok-forum/src/dto/reply.rs",
  sqliteTest: "crates/rustok-forum/tests/read_model_cursor_sqlite.rs",
  postgresTest: "crates/rustok-forum/tests/read_model_cursor_postgres.rs",
  migration:
    "crates/rustok-forum/src/migrations/m20260713_000012_add_forum_read_model_indexes.rs",
};

function fail(message) {
  console.error("forum read-model verification failed:");
  console.error(`- ${message}`);
  process.exit(1);
}

function text(path) {
  if (!existsSync(path)) fail(`${path}: required file is missing`);
  return readFileSync(path, "utf8");
}

function verifyStatic() {
  const dto = text(paths.dto);
  const service = text(paths.service);
  const compatibility = text(paths.compatibility);
  const categoryOwner = text(paths.categoryOwner);
  const servicesRegistry = text(paths.servicesRegistry);
  const topicDto = text(paths.topicDto);
  const replyDto = text(paths.replyDto);
  const migration = text(paths.migration);
  text(paths.sqliteTest);
  text(paths.postgresTest);

  for (const token of [
    "DEFAULT_FORUM_READ_LIMIT: u64 = 20",
    "MAX_FORUM_READ_LIMIT: u64 = 100",
    "CategoryCursorQuery",
    "TopicCursorQuery",
    "ReplyCursorQuery",
    "CategoryCursorPage",
    "TopicCursorPage",
    "ReplyCursorPage",
  ]) {
    if (!dto.includes(token)) fail(`${paths.dto}: missing token ${token}`);
  }

  for (const token of [
    "list_categories",
    "list_topics",
    "list_replies",
    "CATEGORY_CURSOR_VERSION",
    "TOPIC_CURSOR_VERSION",
    "REPLY_CURSOR_VERSION",
    "order_by_asc(forum_category::Column::Position)",
    "order_by_desc(forum_topic::Column::UpdatedAt)",
    "order_by_asc(forum_reply::Column::Position)",
  ]) {
    if (!service.includes(token)) fail(`${paths.service}: missing token ${token}`);
  }

  const overfetches = service.match(/\.limit\(limit \+ 1\)/g) ?? [];
  if (overfetches.length !== 3) {
    fail(`${paths.service}: expected 3 limit+1 keyset overfetches, found ${overfetches.length}`);
  }
  if (service.includes(".paginate(")) {
    fail(`${paths.service}: canonical cursor service must not use offset pagination`);
  }
  for (const token of [
    "idx_forum_categories_cursor",
    "idx_forum_topics_cursor",
    "idx_forum_replies_cursor",
  ]) {
    if (!migration.includes(token)) fail(`${paths.migration}: missing index ${token}`);
  }

  if (!compatibility.includes("bounded_forum_read_limit")) {
    fail(`${paths.compatibility}: topic/reply compatibility APIs are not capped`);
  }
  for (const token of [
    "MAX_FORUM_READ_LIMIT",
    "bounded_forum_read_limit(Some(per_page))",
    "list_paginated_with_locale_fallback",
  ]) {
    if (!categoryOwner.includes(token)) {
      fail(`${paths.categoryOwner}: missing bounded category token ${token}`);
    }
  }
  if (!servicesRegistry.includes("mod category;")) {
    fail(`${paths.servicesRegistry}: raw category persistence module must stay private`);
  }
  if (!servicesRegistry.includes("pub use category_owner::CategoryService;")) {
    fail(`${paths.servicesRegistry}: bounded category owner is not the public export`);
  }
  if (servicesRegistry.includes("pub mod category;")) {
    fail(`${paths.servicesRegistry}: raw category persistence module is publicly reachable`);
  }

  for (const [path, source] of [
    [paths.topicDto, topicDto],
    [paths.replyDto, replyDto],
  ]) {
    if (!source.includes("deserialize_forum_read_limit")) {
      fail(`${path}: external page size is not bounded during deserialization`);
    }
  }

  console.log("forum read-model static verification passed (3 bounded cursor models, max 100)");
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
  if (result.status !== 0) fail(`${label}: exited with status ${result.status}`);
}

verifyStatic();
if (staticOnly) process.exit(0);

run("forum read-model unit and SQLite tests", "cargo", [
  "test",
  "-p",
  "rustok-forum",
  "--test",
  "read_model_cursor_sqlite",
]);

if (
  (process.env.RUSTOK_FORUM_TEST_DATABASE_URL ?? process.env.DATABASE_URL ?? "").startsWith(
    "postgres",
  )
) {
  run("forum read-model PostgreSQL tests", "cargo", [
    "test",
    "-p",
    "rustok-forum",
    "--test",
    "read_model_cursor_postgres",
    "--",
    "--nocapture",
    "--test-threads=1",
  ]);
}

console.log("\nforum read-model verification passed");
