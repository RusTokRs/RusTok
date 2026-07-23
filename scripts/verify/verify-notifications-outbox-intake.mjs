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
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  return readFileSync(absolute, "utf8");
}

function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

function requireOrder(source, markers, message) {
  let previous = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker, previous + 1);
    if (index < 0 || index <= previous) {
      failures.push(`${message}: missing or out-of-order marker ${marker}`);
      return;
    }
    previous = index;
  }
}

function reject(source, pattern, message) {
  if (pattern.test(source)) failures.push(message);
}

const contractPath =
  "crates/rustok-notifications/contracts/notifications-outbox-intake.json";
const contract = JSON.parse(read(contractPath) || "{}");
const migration = read(contract.migration ?? "");
const owner = read(contract.owner_driver ?? "");
const server = read(contract.server_worker ?? "");
const bootstrap = read(contract.bootstrap ?? "");
const forumProvider = read(contract.forum_provider ?? "");
const library = read("crates/rustok-notifications/src/lib.rs");
const manifest = read("crates/rustok-notifications/Cargo.toml");
const test = read(contract.tests?.[0] ?? "");

if (contract.slice !== "NOTIFY-03D") {
  failures.push("outbox intake contract must identify NOTIFY-03D");
}
if (contract.intake?.mutates_general_relay_state !== false) {
  failures.push("Notifications intake must not mutate general outbox relay state");
}
if (contract.intake?.requires_relay_dispatched_status !== false) {
  failures.push("Notifications intake must consume committed envelopes independently of relay delivery");
}
if (contract.intake?.source_inbox_and_receipt_same_transaction !== true) {
  failures.push("source inbox and intake receipt must share one transaction");
}
if (contract.runtime?.default_enabled !== false) {
  failures.push("outbox intake must remain disabled by default");
}
if (contract.selection?.default_batch_size !== 32) {
  failures.push("outbox intake default batch must remain 32");
}
if (contract.selection?.maximum_batch_size !== 64) {
  failures.push("outbox intake hard batch maximum must remain 64");
}

for (const marker of [
  "notification_outbox_intake_receipts",
  "outbox_event_id UUID PRIMARY KEY",
  "outbox_event_id TEXT PRIMARY KEY NOT NULL",
  "ux_notification_source_inbox_tenant_id",
  "FOREIGN KEY (tenant_id, source_inbox_id)",
  "source_revision > 0",
  "idx_notification_outbox_intake_source",
]) {
  requireText(migration, marker, `outbox intake migration is missing ${marker}`);
}

for (const marker of [
  "DEFAULT_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE: usize = 32",
  "MAX_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE: usize = 64",
  "pending_outbox_event_ids",
  "notification_outbox_intake_receipts",
  "not_in_subquery",
  "order_by_asc(outbox_event::Column::CreatedAt)",
  "order_by_asc(outbox_event::Column::Id)",
  "ContractEventEnvelope",
  "ContractEventPayload::ForumMention",
  "ForumMentionEvent::UserMentionAdded",
  "DomainEvent::ForumTopicCreated { topic_id",
  "source_event_ref(envelope.tenant_id, topic_id, FORUM_TOPIC_CREATED, 1)",
  "source_inbox::Entity::insert",
  "intake_receipt::Entity::insert",
  "ensure_source_inbox_identity",
  "ensure_receipt_identity",
]) {
  requireText(owner, marker, `Notifications outbox intake owner is missing ${marker}`);
}
requireOrder(
  owner,
  [
    "let txn = self.db.begin().await?",
    "source_inbox::Entity::insert",
    "intake_receipt::Entity::insert",
    "txn.commit().await?",
  ],
  "source inbox and receipt must commit through one owner transaction",
);
reject(
  owner,
  /rustok_forum::|forum_domain_event::|forum_topic::|forum_user_mention::/,
  "Notifications intake must not read Forum-owned tables or services",
);
reject(
  owner,
  /SysEventStatus|outbox_event::ActiveModel|outbox_event::Entity::update|Column::Status\.eq/,
  "Notifications intake must not gate on or mutate general relay status",
);

for (const marker of [
  "None if event.event_type() == &topic_created_type()",
  "AggregateId.eq(event.event_id())",
  "requested_revision != persisted.sequence_no",
  "TOPIC_CREATED_TYPE if persisted.schema_version == 1 => 1",
  "self.parse_user_mention(&persisted)?.source_revision_id",
]) {
  requireText(forumProvider, marker, `Forum source compatibility is missing ${marker}`);
}

for (const marker of [
  "RUSTOK_NOTIFICATIONS_OUTBOX_INTAKE_ENABLED",
  "runs_background_workers()",
  "NotificationOutboxIntakeWorker::new",
  "StopHandle",
  "pending_outbox_event_ids().await",
  "if *stop_rx.borrow()",
  "worker.process_outbox_event(outbox_event_id).await",
  "tokio::select!",
]) {
  requireText(server, marker, `server outbox intake worker is missing ${marker}`);
}
requireOrder(
  server,
  [
    "outbox_intake_enabled_from_environment()",
    "NotificationOutboxIntakeWorker::new",
    "tokio::spawn",
  ],
  "outbox intake must validate explicit enablement before spawn",
);
requireOrder(
  server,
  [
    "for outbox_event_id in event_ids",
    "if *stop_rx.borrow()",
    "worker.process_outbox_event(outbox_event_id).await",
  ],
  "shutdown must be checked before each next outbox envelope",
);

requireOrder(
  bootstrap,
  [
    "bootstrap_app_runtime",
    "start_notification_outbox_intake_if_enabled",
    "start_notification_candidate_worker_if_ready",
    "connect_runtime_workers_with_runtime",
  ],
  "outbox intake must start after composition and before candidate/runtime workers",
);

for (const marker of [
  "mod outbox_intake;",
  "NotificationOutboxIntakeWorker",
  "DEFAULT_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE",
  "module.migrations().len(), 4",
]) {
  requireText(library, marker, `Notifications facade is missing ${marker}`);
}
for (const marker of ["rustok-events.workspace = true", "rustok-outbox.workspace = true"]) {
  requireText(manifest, marker, `Notifications manifest is missing ${marker}`);
}
for (const marker of [
  "dispatched_root_and_contract_envelopes_enter_source_inbox_once",
  "ContractEventEnvelope::new",
  "ForumMentionEvent::UserMentionAdded",
  "assert_eq!(first.selected, 32)",
  "assert_ne!(topic_outbox_event_id, topic_row.source_event_id)",
  "assert!(replay.replayed)",
  "MAX_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE + 1",
]) {
  requireText(test, marker, `outbox intake SQLite evidence is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Notifications outbox intake verification failed:\n");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Notifications outbox intake boundary verified.");
