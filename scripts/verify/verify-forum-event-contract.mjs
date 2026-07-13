#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";

const args = new Set(process.argv.slice(2));
const allowed = new Set(["--static-only", "--postgres", "--help"]);
for (const arg of args) {
  if (!allowed.has(arg)) fail(`unknown argument: ${arg}`);
}

if (args.has("--help")) {
  console.log(`Usage:
  node scripts/verify/verify-forum-event-contract.mjs [options]

Options:
  --static-only  Validate the event contract inventory without invoking Cargo.
  --postgres     Run the PostgreSQL domain-event integration test.
  --help         Show this help.`);
  process.exit(0);
}

const staticOnly = args.has("--static-only");
const postgres = args.has("--postgres");

const files = {
  entity: "crates/rustok-forum/src/entities/forum_domain_event.rs",
  dto: "crates/rustok-forum/src/dto/event.rs",
  service: "crates/rustok-forum/src/services/event.rs",
  migrationRegistry: "crates/rustok-forum/src/migrations/mod.rs",
  migration:
    "crates/rustok-forum/src/migrations/m20260713_000011_add_forum_domain_events/mod.rs",
  postgresSchema:
    "crates/rustok-forum/src/migrations/m20260713_000011_add_forum_domain_events/postgres_up/schema.rs",
  postgresContent:
    "crates/rustok-forum/src/migrations/m20260713_000011_add_forum_domain_events/postgres_up/content.rs",
  postgresRelations:
    "crates/rustok-forum/src/migrations/m20260713_000011_add_forum_domain_events/postgres_up/relations.rs",
  sqliteSchema:
    "crates/rustok-forum/src/migrations/m20260713_000011_add_forum_domain_events/sqlite_up/schema.rs",
  sqliteCategoryTopic:
    "crates/rustok-forum/src/migrations/m20260713_000011_add_forum_domain_events/sqlite_up/category_topic.rs",
  sqliteReplyRelations:
    "crates/rustok-forum/src/migrations/m20260713_000011_add_forum_domain_events/sqlite_up/reply_relations.rs",
  contract:
    "crates/rustok-forum/tests/support/event_contract.rs",
  sqliteTest: "crates/rustok-forum/tests/domain_event_contract_sqlite.rs",
  postgresTest: "crates/rustok-forum/tests/domain_event_contract_postgres.rs",
};

const eventTypes = [
  "forum.category.created",
  "forum.category.updated",
  "forum.category.deleted",
  "forum.topic.created",
  "forum.topic.updated",
  "forum.topic.deleted",
  "forum.topic.status_changed",
  "forum.topic.pinned_changed",
  "forum.topic.lock_changed",
  "forum.reply.created",
  "forum.reply.updated",
  "forum.reply.deleted",
  "forum.reply.status_changed",
  "forum.solution.marked",
  "forum.solution.unmarked",
  "forum.topic.vote_changed",
  "forum.reply.vote_changed",
  "forum.category.subscription_changed",
  "forum.topic.subscription_changed",
  "forum.topic.tags_changed",
];

function fail(message) {
  console.error("forum event contract verification failed:");
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

function verifyStatic() {
  for (const path of Object.values(files)) {
    if (!existsSync(path)) fail(`${path}: required file is missing`);
  }

  const entity = text(files.entity);
  const service = text(files.service);
  const registry = text(files.migrationRegistry);
  const postgresMigration = [
    text(files.postgresSchema),
    text(files.postgresContent),
    text(files.postgresRelations),
  ].join("\n");
  const sqliteMigration = [
    text(files.sqliteSchema),
    text(files.sqliteCategoryTopic),
    text(files.sqliteReplyRelations),
  ].join("\n");
  const contract = text(files.contract);
  const combinedMigration = `${postgresMigration}\n${sqliteMigration}`;

  for (const token of [
    "sequence_no",
    "event_id",
    "tenant_id",
    "aggregate_type",
    "aggregate_id",
    "event_type",
    "schema_version",
    "actor_id",
    "payload",
    "created_at",
  ]) {
    if (!entity.includes(token)) {
      fail(`${files.entity}: missing event field ${token}`);
    }
  }

  for (const token of [
    "MAX_EVENT_LIMIT: u64 = 100",
    "after_sequence",
    "order_by_asc",
    "TenantId.eq(tenant_id)",
  ]) {
    if (!service.includes(token)) {
      fail(`${files.service}: missing bounded-query token ${token}`);
    }
  }

  if (!registry.includes("m20260713_000011_add_forum_domain_events")) {
    fail(`${files.migrationRegistry}: domain event migration is not registered`);
  }

  for (const eventType of eventTypes) {
    if (!postgresMigration.includes(eventType)) {
      fail(`PostgreSQL migration: missing event type ${eventType}`);
    }
    if (!sqliteMigration.includes(eventType)) {
      fail(`SQLite migration: missing event type ${eventType}`);
    }
    if (!contract.includes(eventType)) {
      fail(`${files.contract}: missing contract assertion ${eventType}`);
    }
  }

  for (const token of [
    "forum_domain_events_immutable_update",
    "forum_domain_events_immutable_delete",
    "idx_forum_domain_events_tenant_sequence",
    "idx_forum_domain_events_tenant_aggregate",
    "idx_forum_domain_events_tenant_type",
  ]) {
    if (!combinedMigration.includes(token)) {
      fail(`domain event migration is missing ${token}`);
    }
  }

  if (combinedMigration.includes("INSERT INTO sys_events")) {
    fail("forum DB triggers must not write untyped payloads into the central sys_events relay");
  }

  console.log(
    `forum event contract static verification passed (${eventTypes.length} event types)`,
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

function requirePostgresUrl() {
  const value =
    process.env.RUSTOK_FORUM_TEST_DATABASE_URL ?? process.env.DATABASE_URL ?? "";
  if (!value.startsWith("postgres://") && !value.startsWith("postgresql://")) {
    fail(
      "--postgres requires RUSTOK_FORUM_TEST_DATABASE_URL (or DATABASE_URL) with a PostgreSQL URL",
    );
  }
}

verifyStatic();
if (staticOnly) process.exit(0);

run("Rust formatting", "cargo", ["fmt", "--all", "--", "--check"]);
run("forum domain event SQLite contract", "cargo", [
  "test",
  "-p",
  "rustok-forum",
  "--test",
  "domain_event_contract_sqlite",
  "--",
  "--nocapture",
]);

if (postgres) {
  requirePostgresUrl();
  run("forum domain event PostgreSQL contract", "cargo", [
    "test",
    "-p",
    "rustok-forum",
    "--test",
    "domain_event_contract_postgres",
    "--",
    "--nocapture",
    "--test-threads=1",
  ]);
}

console.log("\nforum event contract verification passed");
