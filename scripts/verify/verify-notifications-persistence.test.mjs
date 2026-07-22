#!/usr/bin/env node

import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const verifier = path.join(scriptDir, "verify-notifications-persistence.mjs");
const fixtureRoot = mkdtempSync(path.join(tmpdir(), "rustok-notifications-persistence-"));

function write(relativePath, content) {
  const target = path.join(fixtureRoot, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content, "utf8");
}

function run() {
  return spawnSync(process.execPath, [verifier], {
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: fixtureRoot },
    encoding: "utf8",
  });
}

const tables = [
  "notifications",
  "notification_delivery_attempts",
  "notification_fanout_jobs",
  "notification_fanout_items",
  "notification_preferences",
  "notification_digest_jobs",
  "notification_digest_items",
  "notification_push_subscriptions",
];

const validMigration = `
CREATE UNIQUE INDEX IF NOT EXISTS ux_users_tenant_identity ON users (tenant_id, id);
${tables.map((table) => `CREATE TABLE IF NOT EXISTS ${table} (id TEXT);`).join("\n")}
FOREIGN KEY (tenant_id, recipient_id) REFERENCES users(tenant_id, id)
ux_notifications_source_recipient_dedupe
ux_notifications_idempotency
ux_notification_delivery_idempotency
ux_notification_fanout_source
ux_notification_fanout_item_recipient
ux_notification_preference_scope
ux_notification_digest_window
ux_notification_push_endpoint
octet_length(template_data_json::text) <= 8192
length(template_data_json) <= 8192
octet_length(descriptor_json::text) <= 16384
length(descriptor_json) <= 16384
read_at IS NULL OR seen_at IS NOT NULL
status = 'leased'
lease_expires_at IS NOT NULL
encrypted_endpoint
endpoint_hash
notification actor tenant mismatch
notification fanout item tenant mismatch
`;

const validEntities = `
#[sea_orm(table_name = "notifications")] pub struct Notification { pub state: NotificationState }
#[sea_orm(table_name = "notification_delivery_attempts")] pub struct Delivery { pub channel: NotificationChannel, pub status: DeliveryStatus }
#[sea_orm(table_name = "notification_fanout_jobs")] pub struct FanoutJob {}
#[sea_orm(table_name = "notification_fanout_items")] pub struct FanoutItem {}
#[sea_orm(table_name = "notification_preferences")] pub struct Preference { pub delivery_mode: NotificationDeliveryMode }
#[sea_orm(table_name = "notification_digest_jobs")] pub struct DigestJob {}
#[sea_orm(table_name = "notification_digest_items")] pub struct DigestItem {}
#[sea_orm(table_name = "notification_push_subscriptions")] pub struct Push { pub endpoint_hash: String, pub encrypted_endpoint: String }
`;

const files = {
  "crates/rustok-notifications/Cargo.toml": `
    sea-orm.workspace = true
    sea-orm-migration.workspace = true
    serde.workspace = true
  `,
  "crates/rustok-notifications/src/lib.rs": `
    pub mod entities;
    pub mod migrations;
    pub mod model;
    fn migrations() { migrations::migrations(); migrations::migration_dependencies(); }
  `,
  "crates/rustok-notifications/src/model.rs": `
    DeriveActiveEnum
    NotificationState NotificationPriorityValue NotificationChannel DeliveryStatus
    NotificationJobStatus FanoutItemStatus NotificationDeliveryMode DigestMode
    DigestJobStatus PushPlatform PushSubscriptionStatus
  `,
  "crates/rustok-notifications/src/entities.rs": validEntities,
  "crates/rustok-notifications/src/migrations/mod.rs": `
    m20260721_000010_create_notification_persistence
    m20250101_000002_create_users
  `,
  "crates/rustok-notifications/src/migrations/m20260721_000010_create_notification_persistence.rs": validMigration,
  "crates/rustok-notifications/tests/persistence_sqlite.rs": `
    source-event recipient dedupe must hold
    recipient tenant mismatch must fail
    actor tenant mismatch must fail
    read must imply seen
    payload bound must hold
    leased delivery needs lease fields
    push endpoint hash must be normalized
  `,
  "crates/rustok-notifications/tests/persistence_postgres.rs": `
    NOTIFICATIONS_TEST_DATABASE_URL
    CREATE SCHEMA
    DROP SCHEMA IF EXISTS
    pg-cross-tenant-recipient
    pg-cross-tenant-actor
    pg-read-without-seen
    pg-oversized
  `,
  "crates/rustok-notifications/docs/implementation-plan.md": `
    ### Delivered in \`NOTIFY-01A\`
    Remaining: global server migrator registration.
  `,
};

try {
  for (const [relativePath, content] of Object.entries(files)) write(relativePath, content);

  const baseline = run();
  if (baseline.status !== 0) {
    throw new Error(`valid fixture failed:\n${baseline.stdout}\n${baseline.stderr}`);
  }

  write(
    "crates/rustok-notifications/src/migrations/m20260721_000010_create_notification_persistence.rs",
    validMigration.replace("FOREIGN KEY (tenant_id, recipient_id) REFERENCES users(tenant_id, id)", ""),
  );
  const compositeFk = run();
  if (compositeFk.status === 0 || !compositeFk.stderr.includes("FOREIGN KEY (tenant_id, recipient_id)")) {
    throw new Error(`composite-FK fixture did not fail correctly:\n${compositeFk.stdout}\n${compositeFk.stderr}`);
  }
  write(
    "crates/rustok-notifications/src/migrations/m20260721_000010_create_notification_persistence.rs",
    validMigration,
  );

  write(
    "crates/rustok-notifications/src/entities.rs",
    `${validEntities}\npub struct Leak { pub email_address: String, pub raw_payload: String }`,
  );
  const privateData = run();
  if (privateData.status === 0 || !privateData.stderr.includes("forbidden contact/rendered/source-private")) {
    throw new Error(`private-data fixture did not fail correctly:\n${privateData.stdout}\n${privateData.stderr}`);
  }
  write("crates/rustok-notifications/src/entities.rs", validEntities);

  write(
    "crates/rustok-notifications/src/migrations/m20260721_000010_create_notification_persistence.rs",
    `${validMigration}\nendpoint TEXT`,
  );
  const rawEndpoint = run();
  if (rawEndpoint.status === 0 || !rawEndpoint.stderr.includes("encrypted rather than stored raw")) {
    throw new Error(`raw-endpoint fixture did not fail correctly:\n${rawEndpoint.stdout}\n${rawEndpoint.stderr}`);
  }

  console.log("notifications persistence verifier fixtures passed");
} finally {
  rmSync(fixtureRoot, { recursive: true, force: true });
}
