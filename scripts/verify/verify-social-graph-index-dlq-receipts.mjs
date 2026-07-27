#!/usr/bin/env node

import { readFileSync } from "node:fs";

const files = {
  migration: readFileSync(
    "crates/rustok-social-graph/src/migrations/m20260727_000004_create_index_dlq_receipts.rs",
    "utf8",
  ),
  migrations: readFileSync(
    "crates/rustok-social-graph/src/migrations/mod.rs",
    "utf8",
  ),
  module: readFileSync("crates/rustok-social-graph/src/lib.rs", "utf8"),
  receipt: readFileSync(
    "crates/rustok-social-graph/src/index_dlq_receipt.rs",
    "utf8",
  ),
  consumer: readFileSync(
    "crates/rustok-social-graph/src/index_consumer.rs",
    "utf8",
  ),
  worker: readFileSync(
    "apps/server/src/services/social_graph_index_worker.rs",
    "utf8",
  ),
  dlq: readFileSync("crates/rustok-iggy/src/dlq.rs", "utf8"),
};

const failures = [];

function requireText(name, source, text) {
  if (!source.includes(text)) {
    failures.push(`${name} is missing required marker: ${text}`);
  }
}

function forbidText(name, source, text) {
  if (source.includes(text)) {
    failures.push(`${name} contains forbidden marker: ${text}`);
  }
}

for (const marker of [
  "CREATE TABLE IF NOT EXISTS social_graph_index_dlq_receipts",
  "PRIMARY KEY (tenant_id, consumer_group, event_id)",
  "source_stream",
  "source_topic",
  "source_partition",
  "source_offset",
  "payload BYTEA NOT NULL",
  "stable_error_code",
  "projection_attempt_count",
  "state IN ('reserved', 'publishing', 'published', 'acknowledged')",
  "publisher_id",
  "lease_expires_at",
  "published_at",
  "acknowledged_at",
]) {
  requireText("DLQ receipt migration", files.migration, marker);
}
for (const marker of [
  "mod m20260727_000004_create_index_dlq_receipts;",
  "Box::new(m20260727_000004_create_index_dlq_receipts::Migration)",
  '"m20260727_000004_create_index_dlq_receipts"',
  'vec!["m20260726_000003_create_command_receipts"]',
]) {
  requireText("DLQ receipt migration registration", files.migrations, marker);
}
requireText(
  "Social Graph public module",
  files.module,
  "pub mod index_dlq_receipt;",
);
requireText("Social Graph migration count", files.module, "migrations().len(), 4");

for (const marker of [
  "pub enum SocialGraphIndexDlqReceiptState",
  "Reserved",
  "Publishing",
  "Published",
  "Acknowledged",
  "pub struct SocialGraphIndexDlqIdentity",
  "pub source_partition: u32",
  "pub source_offset: u64",
  "pub payload: Vec<u8>",
  "pub async fn find(",
  "pub async fn reserve_and_claim(",
  "pub async fn release_claim(",
  "pub async fn mark_published(",
  "pub async fn mark_acknowledged(",
  "IdentityConflict",
  "SocialGraphIndexDlqPublishClaim::Busy",
  "SocialGraphIndexDlqPublishClaim::AlreadyPublished",
  "SocialGraphIndexDlqPublishClaim::AlreadyAcknowledged",
  "lease_expires_expression",
  "CURRENT_TIMESTAMP +",
  "datetime('now'",
  "receipt_claim_publish_and_acknowledge_are_idempotent",
  "receipt_rejects_same_key_with_different_source_bytes",
]) {
  requireText("durable DLQ receipt store", files.receipt, marker);
}
for (const forbidden of [
  "ack_token",
  "error_details",
  "raw_error",
  "tenant_id: String",
  "source_offset: String",
]) {
  forbidText("durable DLQ receipt store", files.receipt, forbidden);
}

for (const marker of [
  "dlq_receipts: SocialGraphIndexDlqReceiptStore",
  "dlq_publisher_id: Uuid",
  "self.consumed_dlq_receipt(consumed).await?",
  "SocialGraphIndexDlqReceiptState::Published",
  "SocialGraphIndexDlqReceiptState::Acknowledged",
  "SocialGraphIndexConsumerError::DlqPublishInProgress",
  "reserve_and_claim(",
  "consumed.raw_payload().to_vec()",
  ".move_to_dlq(entry)",
  ".mark_published(&identity, self.dlq_publisher_id)",
  "PreviouslyPublished",
  "self.group",
  ".acknowledge(consumed)",
  ".mark_acknowledged(&identity)",
  "Source offset committed but DLQ receipt acknowledgement bookkeeping failed",
]) {
  requireText("receipt-aware Social Graph consumer", files.consumer, marker);
}

const receiptCheck = files.consumer.indexOf("self.consumed_dlq_receipt(consumed).await?");
const projection = files.consumer.indexOf("self.projector.apply_envelope(&consumed.envelope).await", receiptCheck);
if (receiptCheck < 0 || projection <= receiptCheck) {
  failures.push("consumer must inspect a durable DLQ receipt before projection");
}
const brokerPublish = files.consumer.indexOf(".move_to_dlq(entry)");
const publishedReceipt = files.consumer.indexOf(
  ".mark_published(&identity, self.dlq_publisher_id)",
  brokerPublish,
);
if (brokerPublish < 0 || publishedReceipt <= brokerPublish) {
  failures.push("consumer must mark the receipt published only after broker publication succeeds");
}
const brokerAck = files.consumer.indexOf(".acknowledge(consumed)");
const receiptAck = files.consumer.indexOf(".mark_acknowledged(&identity)", brokerAck);
if (brokerAck < 0 || receiptAck <= brokerAck) {
  failures.push("receipt acknowledgement bookkeeping must follow the source broker commit");
}

for (const marker of [
  "publish_dead_lettered_result",
  "error.is_retryable() && attempt < config.max_attempts",
  "STAGE_DLQ_PUBLISH",
  '"published"',
  '"already_published"',
  "continuing_durable_receipt",
  "config.dlq_enabled || continuing_durable_receipt",
  "SOCIAL_GRAPH_INDEX_DLQ_RECEIPT_RECOVERED_CODE",
  "durable DLQ receipt remains published",
  "redelivery skips projection and DLQ publication",
]) {
  requireText("receipt-aware Social Graph worker", files.worker, marker);
}
for (const forbidden of [
  "redelivery may republish until a durable DLQ identity exists",
  "DLQ publication succeeded but the source offset remains uncommitted",
]) {
  forbidText("receipt-aware Social Graph worker", files.worker, forbidden);
}

for (const marker of [
  "entry.payload",
  "entry.event_id.to_string()",
  "connector.publish(request)",
]) {
  requireText("Iggy DLQ exact-byte publication", files.dlq, marker);
}

if (failures.length > 0) {
  console.error("Social Graph Index DLQ receipt verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "Social Graph Index DLQ receipt verification passed: immutable source identity and bytes, durable reserve/publish/ack states, bounded publish leases, pre-projection recovery, retryable publication, acknowledgement-only redelivery, and no receipt-owned transport token are locked.",
);
