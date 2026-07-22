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
  "crates/rustok-notifications/contracts/notifications-source-fanout.json";
const contract = JSON.parse(read(contractPath) || "{}");
const migration = read(contract.migration ?? "");
const service = read(contract.service ?? "");
const entities = read("crates/rustok-notifications/src/entities.rs");
const model = read("crates/rustok-notifications/src/model.rs");
const library = read("crates/rustok-notifications/src/lib.rs");
const ownerTest = read("crates/rustok-notifications/tests/fanout_sqlite.rs");
const forumProvider = read(
  contract.forum_user_mention_source?.provider ?? "",
);
const forumTest = read("crates/rustok-forum/tests/notification_source_sqlite.rs");

if (contract.slice !== "NOTIFY-01B/03A") {
  failures.push("machine contract must identify NOTIFY-01B/03A");
}
if (contract.candidate_items?.creates_notification_rows !== false) {
  failures.push("candidate fan-out must not claim final notification creation");
}
if (contract.candidate_items?.creates_delivery_attempts !== false) {
  failures.push("candidate fan-out must not claim delivery enqueue");
}
if (contract.source_inbox?.changed_event_type_or_revision_conflicts !== true) {
  failures.push("source inbox must reject changed event identity replay");
}
if (contract.source_inbox?.expired_lease_cannot_finish !== true) {
  failures.push("source inbox must reject completion under an expired lease");
}
for (const field of [
  "one_descriptor_job_per_source_event",
  "provider_page_must_not_exceed_requested_limit",
  "empty_page_cannot_continue",
  "cursor_must_advance",
  "lease_recovery",
  "expired_lease_cannot_finish",
  "descriptor_replay_must_match",
]) {
  if (contract.fanout_job?.[field] !== true) {
    failures.push(`fan-out contract must set fanout_job.${field}=true`);
  }
}
if (contract.fanout_job?.audience_page_max !== 256) {
  failures.push("fan-out page bound must remain 256");
}

for (const marker of [
  "CREATE TABLE IF NOT EXISTS notification_source_inbox",
  "ux_notification_source_inbox_event",
  "tenant_id, source_slug, source_event_id",
  "status IN ('pending', 'processing', 'completed', 'suppressed', 'retryable_error', 'rejected')",
  "ON DELETE RESTRICT",
  "lease_expires_at",
  "source_revision > 0",
]) {
  requireText(migration, marker, `source inbox migration is missing ${marker}`);
}
reject(
  migration,
  /source_event_id,\s*event_type/,
  "source-event dedupe must not split one event id by event type",
);

for (const marker of [
  "NotificationSourceInboxStatus",
  "Pending",
  "Processing",
  "Completed",
  "Suppressed",
  "RetryableError",
  "Rejected",
]) {
  requireText(model, marker, `source inbox status model is missing ${marker}`);
}
for (const marker of [
  'table_name = "notification_source_inbox"',
  "pub mod source_inbox",
  "pub fanout_job_id: Option<Uuid>",
]) {
  requireText(entities, marker, `source inbox entity is missing ${marker}`);
}

for (const marker of [
  "pub struct NotificationFanoutService",
  "enqueue_source_event",
  "materialize_source_event",
  "process_fanout_page",
  "SourceIdentityConflict",
  "provider_for_event",
  "describe_event",
  "resolve_audience",
  "CursorDidNotAdvance",
  "NotificationJobStatus::Leased",
  "FanoutItemStatus::Pending",
  "exec_without_returning",
  "MAX_FANOUT_PAGE_SIZE: u16 = 256",
  "recipients.len() > usize::from(limit)",
  "recipients.is_empty() && next_cursor.is_some()",
  "LeaseExpiresAt.gt(timestamp)",
  "lease_expires_at",
  "job.notification_type != notification_type",
]) {
  requireText(service, marker, `fan-out owner service is missing ${marker}`);
}
requireOrder(
  service,
  [
    "enqueue_source_event",
    "materialize_source_event",
    "find_or_create_job",
    "process_fanout_page",
    "persist_fanout_page",
  ],
  "source intake, materialization and page persistence must remain explicit phases",
);
requireOrder(
  service,
  [
    "claim_job(job_id, worker_id)",
    "resolve_audience",
    "persist_fanout_page",
    "NotificationJobStatus::Completed",
  ],
  "fan-out page must resolve under a lease before cursor completion",
);
reject(
  service,
  /notification::ActiveModel|notification_delivery_attempt|DeliveryStatus::Pending/,
  "03A must not bypass preference/privacy by creating notifications or deliveries",
);
reject(
  service,
  /Vec<Uuid>.*descriptor|descriptor.*recipient_ids/s,
  "source descriptors must not carry unbounded recipient lists",
);

for (const marker of [
  "NotificationFanoutService",
  "NotificationFanoutPageResult",
  "NotificationSourceInboxReceipt",
  "NotificationError",
  "module.migrations().len(), 3",
]) {
  requireText(library, marker, `notifications public owner facade is missing ${marker}`);
}

for (const marker of [
  "source_inbox_and_bounded_candidate_fanout_are_idempotent",
  "NOTIFICATION_SOURCE_IDENTITY_CONFLICT",
  "first_page.next_cursor.as_deref(), Some(PAGE_TWO)",
  "FanoutItemStatus::Pending",
  "notification_count, 0",
]) {
  requireText(ownerTest, marker, `fan-out SQLite scenario is missing ${marker}`);
}

for (const marker of [
  "forum.mention.user_added",
  "ForumUserMentionPayload",
  "user_mention_relation_exists",
  "forum_user_mention::Entity::find",
  "load_public_target",
  "ReplyStatus::Pending",
  "NotificationProviderError::Internal { retryable: true }",
  "event.actor_id == Some(payload.mentioned_user_id)",
  "forum_reply_target_kind",
]) {
  requireText(forumProvider, marker, `Forum mention provider is missing ${marker}`);
}
reject(
  forumProvider,
  /forum\.mention\.audience_added/,
  "moderator audience expansion must remain deferred until an owner directory port exists",
);
reject(
  forumProvider,
  /rustok_notifications::|notification_fanout_jobs|notification_source_inbox/,
  "Forum provider must depend only on the neutral notifications API",
);

for (const marker of [
  "seed_user_mention_event",
  "forum.mention.user_added",
  "mention_page.recipients()[0].recipient_id",
  "closed_mention_page.recipients().is_empty()",
]) {
  requireText(forumTest, marker, `Forum mention source scenario is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Notifications source fan-out verification failed:\n");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Notifications source fan-out boundary verified.");
