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
const receiptMigration = read(contract.receipt_migration ?? "");
const rejectionMigration = read(contract.rejection_migration ?? "");
const owner = read(contract.owner_driver ?? "");
const server = read(contract.server_worker ?? "");
const bootstrap = read(contract.bootstrap ?? "");
const forumProvider = read(contract.forum_provider ?? "");
const library = read("crates/rustok-notifications/src/lib.rs");
const manifest = read("crates/rustok-notifications/Cargo.toml");
const test = read(contract.tests?.[0] ?? "");

if (contract.slice !== "NOTIFY-03D" || contract.schema_version !== 2) {
  failures.push("outbox intake contract must identify hardened NOTIFY-03D schema 2");
}
for (const [field, expected, message] of [
  ["mutates_general_relay_state", false, "intake must not mutate relay state"],
  ["requires_relay_dispatched_status", false, "intake must be relay-status independent"],
  ["reads_producer_private_tables", false, "intake must not read producer tables"],
  ["owner_imports_event_or_outbox_crates", false, "owner must not import event/outbox crates"],
  ["decoder_injected_by_executable_host", true, "decoder must be host injected"],
  ["permanent_failure_quarantined", true, "permanent failures must be quarantined"],
  ["retryable_failure_has_no_terminal_record", true, "retryable failures must remain retryable"],
  ["accepted_and_rejected_mutually_exclusive", true, "terminal outcomes must be exclusive"],
]) {
  if (contract.intake?.[field] !== expected) failures.push(message);
}
if (contract.runtime?.default_enabled !== false) {
  failures.push("outbox intake must remain disabled by default");
}
if (contract.selection?.default_batch_size !== 32 || contract.selection?.maximum_batch_size !== 64) {
  failures.push("outbox intake batch bounds must remain 32/64");
}
if (contract.selection?.anti_joins_receipts !== true || contract.selection?.anti_joins_rejections !== true) {
  failures.push("selection must exclude both terminal outcome tables");
}

for (const marker of [
  "notification_outbox_intake_receipts",
  "outbox_event_id UUID PRIMARY KEY",
  "outbox_event_id TEXT PRIMARY KEY NOT NULL",
  "FOREIGN KEY (tenant_id, source_inbox_id)",
  "source_revision > 0",
]) {
  requireText(receiptMigration, marker, `receipt migration is missing ${marker}`);
}
for (const marker of [
  "notification_outbox_intake_rejections",
  "REFERENCES sys_events(id) ON DELETE CASCADE",
  "notification_outbox_intake_receipt_terminal_guard_insert",
  "notification_outbox_intake_rejection_terminal_guard_insert",
  "pg_advisory_xact_lock",
  "RAISE(ABORT",
]) {
  requireText(rejectionMigration, marker, `rejection migration is missing ${marker}`);
}

for (const marker of [
  "pub trait NotificationOutboxEnvelopeDecoder",
  "NotificationOutboxEnvelopeRecord",
  "NotificationOutboxIntakeOutcome",
  "notification_outbox_intake_receipts",
  "notification_outbox_intake_rejections",
  "not_in_subquery(receipts)",
  "not_in_subquery(rejections)",
  "order_by_asc(outbox_event::Column::CreatedAt)",
  "source_inbox::Entity::insert",
  "intake_receipt::Entity::insert",
  "persist_rejection",
  "error.is_retryable()",
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
  /rustok_events|rustok_outbox|rustok_forum::|forum_domain_event::|forum_topic::|forum_user_mention::/,
  "Notifications owner must not decode platform envelopes or read producer state",
);
reject(
  owner,
  /SysEventStatus|outbox_event::ActiveModel|outbox_event::Entity::update|Column::Status\.eq/,
  "Notifications intake must not gate on or mutate relay status",
);

for (const marker of [
  "ServerNotificationOutboxEnvelopeDecoder",
  "ContractEventEnvelope",
  "ContractEventPayload::ForumMention",
  "ForumMentionEvent::UserMentionAdded",
  "DomainEvent::ForumTopicCreated { topic_id",
  "NotificationOutboxIntakeOutcome::Accepted",
  "NotificationOutboxIntakeOutcome::Rejected",
  "RUSTOK_NOTIFICATIONS_OUTBOX_INTAKE_ENABLED",
  "runs_background_workers()",
  "StopHandle",
  "if *stop_rx.borrow()",
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
  "outbox intake must validate enablement before spawn",
);
requireOrder(
  server,
  [
    "for outbox_event_id in event_ids",
    "if *stop_rx.borrow()",
    "worker.process_outbox_event(outbox_event_id).await",
  ],
  "shutdown must be checked before each envelope",
);

requireOrder(
  bootstrap,
  [
    "bootstrap_app_runtime",
    "start_notification_outbox_intake_if_enabled",
    "start_notification_candidate_worker_if_ready",
    "connect_runtime_workers_with_runtime",
  ],
  "outbox intake must start after composition and before later workers",
);

for (const marker of [
  "NotificationOutboxEnvelopeDecoder",
  "NotificationOutboxIntakeOutcome",
  "module.migrations().len(), 5",
]) {
  requireText(library, marker, `Notifications facade is missing ${marker}`);
}
reject(manifest, /rustok-events|rustok-outbox|rustok-api\.workspace/, "owner manifest must remain lock-compatible and neutral");

for (const marker of [
  "accepted_and_permanent_invalid_envelopes_leave_no_head_of_line_blocker",
  "assert_eq!(first.accepted, 31)",
  "assert_eq!(first.rejected, 1)",
  "NotificationOutboxIntakeOutcome::Rejected",
  "retryable event remains claimable",
  "MAX_NOTIFICATION_OUTBOX_INTAKE_BATCH_SIZE + 1",
]) {
  requireText(test, marker, `outbox intake SQLite evidence is missing ${marker}`);
}

for (const marker of [
  "None if event.event_type() == &topic_created_type()",
  "AggregateId.eq(event.event_id())",
  "requested_revision != persisted.sequence_no",
  "TOPIC_CREATED_TYPE if persisted.schema_version == 1 => 1",
]) {
  requireText(forumProvider, marker, `Forum source compatibility is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Notifications outbox intake verification failed:\n");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Notifications hardened outbox intake boundary verified.");
