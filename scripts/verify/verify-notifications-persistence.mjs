#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const filePath = path.join(repoRoot, relativePath);
  if (!existsSync(filePath)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  return readFileSync(filePath, "utf8");
}

function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

function reject(source, pattern, message) {
  if (pattern.test(source)) failures.push(message);
}

const cargoPath = "crates/rustok-notifications/Cargo.toml";
const libPath = "crates/rustok-notifications/src/lib.rs";
const modelPath = "crates/rustok-notifications/src/model.rs";
const entitiesPath = "crates/rustok-notifications/src/entities.rs";
const migrationRegistryPath = "crates/rustok-notifications/src/migrations/mod.rs";
const migrationPath = "crates/rustok-notifications/src/migrations/m20260721_000010_create_notification_persistence.rs";
const sqliteTestPath = "crates/rustok-notifications/tests/persistence_sqlite.rs";
const postgresTestPath = "crates/rustok-notifications/tests/persistence_postgres.rs";
const localPlanPath = "crates/rustok-notifications/docs/implementation-plan.md";

const cargo = read(cargoPath);
const lib = read(libPath);
const model = read(modelPath);
const entities = read(entitiesPath);
const migrationRegistry = read(migrationRegistryPath);
const migration = read(migrationPath);
const sqliteTest = read(sqliteTestPath);
const postgresTest = read(postgresTestPath);
const localPlan = read(localPlanPath);

for (const marker of [
  "sea-orm.workspace = true",
  "sea-orm-migration.workspace = true",
  "serde.workspace = true",
]) {
  requireText(cargo, marker, `${cargoPath}: missing persistence dependency ${marker}`);
}

for (const marker of [
  "pub mod entities;",
  "pub mod migrations;",
  "pub mod model;",
  "migrations::migrations()",
  "migrations::migration_dependencies()",
]) {
  requireText(lib, marker, `${libPath}: missing owner persistence export ${marker}`);
}

for (const marker of [
  "NotificationState",
  "NotificationPriorityValue",
  "NotificationChannel",
  "DeliveryStatus",
  "NotificationJobStatus",
  "FanoutItemStatus",
  "NotificationDeliveryMode",
  "DigestMode",
  "DigestJobStatus",
  "PushPlatform",
  "PushSubscriptionStatus",
  "DeriveActiveEnum",
]) {
  requireText(model, marker, `${modelPath}: missing typed persistence value ${marker}`);
}

for (const marker of [
  'table_name = "notifications"',
  'table_name = "notification_delivery_attempts"',
  'table_name = "notification_fanout_jobs"',
  'table_name = "notification_fanout_items"',
  'table_name = "notification_preferences"',
  'table_name = "notification_digest_jobs"',
  'table_name = "notification_digest_items"',
  'table_name = "notification_push_subscriptions"',
  "pub state: NotificationState",
  "pub channel: NotificationChannel",
  "pub status: DeliveryStatus",
  "pub delivery_mode: NotificationDeliveryMode",
  "pub endpoint_hash: String",
  "pub encrypted_endpoint: String",
]) {
  requireText(entities, marker, `${entitiesPath}: missing typed entity field ${marker}`);
}
reject(entities, /pub\s+(?:state|status|channel|priority|delivery_mode|digest_mode|platform):\s+String/, `${entitiesPath}: typed persistence state regressed to raw String`);
reject(entities, /email_address|phone_number|rendered_html|raw_payload|source_payload/i, `${entitiesPath}: owner entities persist forbidden contact/rendered/source-private data`);

requireText(migrationRegistry, "m20260721_000010_create_notification_persistence", `${migrationRegistryPath}: persistence migration is not registered`);
requireText(migrationRegistry, "m20250101_000002_create_users", `${migrationRegistryPath}: users dependency is not declared`);

for (const marker of [
  "CREATE UNIQUE INDEX IF NOT EXISTS ux_users_tenant_identity",
  "FOREIGN KEY (tenant_id, recipient_id)",
  "REFERENCES users(tenant_id, id)",
  "ux_notifications_source_recipient_dedupe",
  "ux_notifications_idempotency",
  "ux_notification_delivery_idempotency",
  "ux_notification_fanout_source",
  "ux_notification_fanout_item_recipient",
  "ux_notification_preference_scope",
  "ux_notification_digest_window",
  "ux_notification_push_endpoint",
  "octet_length(template_data_json::text) <= 8192",
  "length(template_data_json) <= 8192",
  "octet_length(descriptor_json::text) <= 16384",
  "length(descriptor_json) <= 16384",
  "read_at IS NULL OR seen_at IS NOT NULL",
  "status = 'leased'",
  "lease_expires_at IS NOT NULL",
  "encrypted_endpoint",
  "endpoint_hash",
  "notification actor tenant mismatch",
  "notification fanout item tenant mismatch",
]) {
  requireText(migration, marker, `${migrationPath}: missing database invariant ${marker}`);
}

for (const table of [
  "notifications",
  "notification_delivery_attempts",
  "notification_fanout_jobs",
  "notification_fanout_items",
  "notification_preferences",
  "notification_digest_jobs",
  "notification_digest_items",
  "notification_push_subscriptions",
]) {
  requireText(migration, `CREATE TABLE IF NOT EXISTS ${table}`, `${migrationPath}: missing table ${table}`);
}

reject(migration, /\b(?:email_address|phone_number|rendered_html|raw_payload|source_payload)\b/i, `${migrationPath}: migration persists forbidden contact/rendered/source-private data`);
reject(migration, /\bendpoint\s+(?:TEXT|VARCHAR)/i, `${migrationPath}: push endpoint must be encrypted rather than stored raw`);

for (const [source, pathName, markers] of [
  [sqliteTest, sqliteTestPath, [
    "source-event recipient dedupe must hold",
    "recipient tenant mismatch must fail",
    "actor tenant mismatch must fail",
    "read must imply seen",
    "payload bound must hold",
    "leased delivery needs lease fields",
    "push endpoint hash must be normalized",
  ]],
  [postgresTest, postgresTestPath, [
    "NOTIFICATIONS_TEST_DATABASE_URL",
    "CREATE SCHEMA",
    "DROP SCHEMA IF EXISTS",
    "pg-cross-tenant-recipient",
    "pg-cross-tenant-actor",
    "pg-read-without-seen",
    "pg-oversized",
  ]],
]) {
  for (const marker of markers) {
    requireText(source, marker, `${pathName}: missing executable persistence evidence ${marker}`);
  }
}

requireText(localPlan, "Delivered in `NOTIFY-01A`", `${localPlanPath}: NOTIFY-01A delivery is not recorded`);
requireText(localPlan, "global server migrator registration", `${localPlanPath}: remaining global composition work is not explicit`);

if (failures.length > 0) {
  console.error("notifications persistence verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("notifications persistence verification passed");
