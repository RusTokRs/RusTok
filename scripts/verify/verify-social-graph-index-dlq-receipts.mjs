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
  messageId: readFileSync(
    "crates/rustok-social-graph/src/index_dlq_message_id.rs",
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
  dlqPublisher: readFileSync(
    "crates/rustok-iggy/src/dlq_publisher.rs",
    "utf8",
  ),
  transport: readFileSync("crates/rustok-iggy/src/transport.rs", "utf8"),
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
for (const marker of [
  "mod index_dlq_message_id;",
  "pub mod index_dlq_receipt;",
  "migrations().len(), 4",
]) {
  requireText("Social Graph module", files.module, marker);
}

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
  'b"rustok.social_graph.index.dlq.message_id.v1"',
  "Sha256::new()",
  "identity.tenant_id.as_bytes()",
  "identity.consumer_group.as_bytes()",
  "identity.event_id.as_bytes()",
  "identity.source_stream.as_bytes()",
  "identity.source_topic.as_bytes()",
  "identity.source_partition.to_be_bytes()",
  "identity.source_offset.to_be_bytes()",
  "identity.payload",
  "bytes[6] = (bytes[6] & 0x0f) | 0x80",
  "bytes[8] = (bytes[8] & 0x3f) | 0x80",
  "broker_message_id_is_stable_and_custom_versioned",
  "broker_message_id_changes_with_exact_payload_or_source_position",
]) {
  requireText("deterministic DLQ message identity", files.messageId, marker);
}
for (const forbidden of ["Uuid::new_v4", "DefaultHasher", "SystemTime", "retry_count"]) {
  forbidText("deterministic DLQ message identity", files.messageId, forbidden);
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
  "social_graph_index_dlq_broker_message_id(&identity)",
  ".with_broker_message_id(broker_message_id)",
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
const projection = files.consumer.indexOf(
  "self.projector.apply_envelope(&consumed.envelope).await",
  receiptCheck,
);
if (receiptCheck < 0 || projection <= receiptCheck) {
  failures.push("consumer must inspect a durable DLQ receipt before projection");
}
const brokerId = files.consumer.indexOf("social_graph_index_dlq_broker_message_id(&identity)");
const brokerPublish = files.consumer.indexOf(".move_to_dlq(entry)");
const publishedReceipt = files.consumer.indexOf(
  ".mark_published(&identity, self.dlq_publisher_id)",
  brokerPublish,
);
if (brokerId < 0 || brokerPublish <= brokerId || publishedReceipt <= brokerPublish) {
  failures.push("consumer must derive the broker ID, publish, then mark the durable receipt published");
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
  "broker_message_id: Option<Uuid>",
  "with_broker_message_id",
  "broker_message_id(&self)",
  "publish_event_id = entry.broker_message_id.unwrap_or(entry.event_id)",
  "entry.payload",
  "entry.event_id.to_string()",
]) {
  requireText("Iggy DLQ envelope", files.dlq, marker);
}

for (const marker of [
  "pub(crate) struct IggyDlqPublisher",
  "IggyClient::from_connection_string",
  "partition_for_message_id(message_id, self.partitions)",
  "message_id.as_u128() % u128::from(partitions)",
  "Partitioning::partition_id(partition)",
  "IggyMessage::builder()",
  ".id(message_id.as_u128())",
  ".payload(entry.payload.clone().into())",
  ".send(vec![message])",
  "deterministic DLQ broker message ID is required",
  "deterministic_partition_is_stable_and_one_based",
  "deterministic_partition_changes_only_with_id_or_partition_count",
  "connection_strings",
]) {
  requireText("identified Iggy DLQ publisher", files.dlqPublisher, marker);
}
for (const forbidden of [
  "Uuid::new_v4",
  "DefaultHasher",
  ".id(0)",
  "message_id = 0",
  "entry.retry_count %",
]) {
  forbidText("identified Iggy DLQ publisher", files.dlqPublisher, forbidden);
}

for (const marker of [
  "dlq_publisher: Mutex<Option<IggyDlqPublisher>>",
  "if entry.broker_message_id().is_some()",
  "IggyDlqPublisher::connect(&self.config)",
  ".publish(&entry)",
  "*publisher = None",
  "dropping SDK client for reconnect",
]) {
  requireText("Iggy transport DLQ publisher lifecycle", files.transport, marker);
}

if (failures.length > 0) {
  console.error("Social Graph Index DLQ receipt verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "Social Graph Index DLQ receipt verification passed: immutable source identity and bytes, deterministic UUIDv8 broker IDs, stable per-ID partitions, explicit Iggy u128 headers, durable reserve/publish/ack states, bounded publish leases, pre-projection recovery, reconnectable publication, acknowledgement-only redelivery, and no receipt-owned transport token are locked.",
);
