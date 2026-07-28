#!/usr/bin/env node

import { readFileSync } from "node:fs";

const files = {
  cargo: readFileSync("crates/rustok-iggy-connector/Cargo.toml", "utf8"),
  migrations: readFileSync(
    "crates/rustok-iggy-connector/src/migrations.rs",
    "utf8",
  ),
  receipts: readFileSync(
    "crates/rustok-iggy-connector/src/consumer_poison_receipt.rs",
    "utf8",
  ),
  decodeFailure: readFileSync(
    "crates/rustok-iggy/src/contract_decode_failure.rs",
    "utf8",
  ),
  contractCursor: readFileSync(
    "crates/rustok-iggy/src/contract_consumer.rs",
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
  'sea-orm = { workspace = true, optional = true }',
  'migrations = ["dep:sea-orm", "dep:sea-orm-migration"]',
]) {
  requireText("Iggy connector durable-storage feature", files.cargo, marker);
}

for (const marker of [
  "m20260728_000001_create_consumer_poison_receipts::Migration",
  "CREATE TABLE IF NOT EXISTS iggy_consumer_poison_receipts",
  "CONSTRAINT uq_iggy_consumer_poison_source UNIQUE",
  "payload BYTEA NOT NULL",
  "payload BLOB NOT NULL",
  "state IN ('reserved', 'publishing', 'published', 'acknowledged')",
  "idx_iggy_consumer_poison_state",
]) {
  requireText("Iggy consumer poison migration", files.migrations, marker);
}
for (const forbidden of [
  "octet_length(payload) > 0",
  "CHECK (length(payload) > 0)",
]) {
  forbidText("exact payload migration", files.migrations, forbidden);
}

for (const marker of [
  "pub struct ConsumerPoisonIdentity",
  "delivery_id: Uuid",
  "consumer_group: String",
  "source_stream: String",
  "source_topic: String",
  "source_partition: u32",
  "source_offset: u64",
  "payload: Vec<u8>",
  "pub const fn delivery_id(&self) -> Uuid",
  "pub const fn source_offset(&self) -> u64",
  "pub fn payload(&self) -> &[u8]",
  "pub struct ConsumerPoisonReceiptStore",
  "pub async fn find(",
  "pub async fn reserve_and_claim",
  "pub async fn release_claim",
  "pub async fn mark_published",
  "pub async fn mark_acknowledged",
  "ConsumerPoisonPublishClaim::AlreadyPublished",
  "ConsumerPoisonPublishClaim::AlreadyAcknowledged",
  '"iggy.connector.poison_identity_conflict"',
  "first_delivery_attempt_count",
  "source_offset_i64",
  "stored_payload != identity.payload",
  "select_receipt_by_delivery_id_sql",
  "lease_expires_at <= CURRENT_TIMESTAMP",
  "retained.stable_error_code",
  "retained.first_delivery_attempt_count",
  "identity_is_immutable_and_empty_payload_is_valid",
  "same_delivery_id_rejects_different_source_coordinates",
]) {
  requireText("Iggy consumer poison receipt store", files.receipts, marker);
}

const productionReceipts = files.receipts.split("#[cfg(test)]")[0];
for (const forbidden of [
  "pub delivery_id:",
  "pub consumer_group:",
  "pub source_stream:",
  "pub source_topic:",
  "pub source_partition:",
  "pub source_offset:",
  "pub payload:",
  "payload.is_empty()",
  "tenant_id",
  "event_id",
  "DomainEvent",
  "ContractEventEnvelope",
  "acknowledge(",
  "move_to_dlq(",
  "Uuid::new_v4()",
  "receipt.stable_error_code != stable_error_code",
  "receipt.delivery_attempt_count !=",
  "observed_delivery_attempt_count == receipt",
]) {
  forbidText("neutral poison receipt production code", productionReceipts, forbidden);
}

for (const marker of [
  "pub fn delivery_id(&self) -> Uuid",
  "pub fn raw_payload(&self) -> &[u8]",
  "pub const fn offset(&self) -> u64",
  "pub const fn stable_error_code(&self) -> &'static str",
  "pub fn to_dlq_entry(&self, retry_count: u32)",
]) {
  requireText("raw decode-failure bridge facts", files.decodeFailure, marker);
}

for (const marker of [
  "pub enum PersistentContractDelivery",
  "PersistentContractDelivery::DecodeFailure",
  "pub async fn receive_delivery",
  "pub async fn acknowledge_decode_failure",
]) {
  requireText("typed contract cursor", files.contractCursor, marker);
}

for (const marker of [
  "pub async fn receive_delivery(",
  "self.group",
  ".receive_delivery()",
  "PersistentContractDelivery::DecodeFailure",
  "pub async fn acknowledge_decode_failure(",
  ".acknowledge_decode_failure(consumed)",
]) {
  requireText("Social Graph typed delivery adapter", files.consumer, marker);
}

for (const marker of [
  "PersistentContractDelivery::DecodeFailure(failure)",
  "ConsumerPoisonReceiptStore::new(ctx.db_clone())",
  "ConsumerPoisonIdentity::new(",
  "poison_receipts.find(&identity).await",
  "!config.dlq_enabled && !continuing_durable_receipt",
  ".reserve_and_claim(",
  "transport.move_to_dlq(failure.to_dlq_entry(1)).await",
  "mark_raw_poison_published(",
  ".mark_published(identity, poison_publisher_id)",
  "acknowledge_raw_poison_result(",
  "consumer.acknowledge_decode_failure(failure).await",
  "poison_receipts.mark_acknowledged(identity).await",
  "retrying acknowledgement only",
  "undecodable broker offset uncommitted",
]) {
  requireText("Social Graph raw poison worker", files.worker, marker);
}

const deliveryLookup = files.receipts.indexOf(
  "select_receipt_by_delivery_id_sql(backend, true)",
);
const insert = files.receipts.indexOf("insert_receipt_sql(backend)", deliveryLookup);
const sourceSelect = files.receipts.indexOf(
  "select_receipt_by_source_sql(backend, true)",
  insert,
);
const claim = files.receipts.indexOf("claim_receipt_sql(backend)", sourceSelect);
if (
  deliveryLookup < 0 ||
  insert <= deliveryLookup ||
  sourceSelect <= insert ||
  claim <= sourceSelect
) {
  failures.push(
    "receipt reservation must validate delivery UUID, persist, reload exact source identity, and only then claim publication",
  );
}

const rawStart = files.worker.indexOf("async fn process_decode_failure(");
const rawEnd = files.worker.indexOf("async fn acknowledge_terminal_result(", rawStart);
const rawFlow =
  rawStart >= 0 && rawEnd > rawStart
    ? files.worker.slice(rawStart, rawEnd)
    : "";
const existingLookup = rawFlow.indexOf("poison_receipts.find(&identity).await");
const disabledGate = rawFlow.indexOf(
  "!config.dlq_enabled && !continuing_durable_receipt",
);
const rawReserve = rawFlow.indexOf(".reserve_and_claim(");
const rawPublish = rawFlow.indexOf(
  "transport.move_to_dlq(failure.to_dlq_entry(1)).await",
);
const rawMarkCall = rawFlow.indexOf("mark_raw_poison_published(", rawPublish);
const rawAcknowledgeCall = rawFlow.indexOf(
  "acknowledge_raw_poison_result(",
  rawMarkCall,
);
if (
  existingLookup < 0 ||
  disabledGate <= existingLookup ||
  rawReserve <= disabledGate ||
  rawPublish <= rawReserve ||
  rawMarkCall <= rawPublish ||
  rawAcknowledgeCall <= rawMarkCall
) {
  failures.push(
    "raw poison flow must recognize an existing durable choice, reject only new disabled-DLQ work, reserve, publish exact bytes, persist published, and only then enter acknowledgement",
  );
}

const markStart = files.worker.indexOf("async fn mark_raw_poison_published(");
const ackStart = files.worker.indexOf("async fn acknowledge_raw_poison_result(");
const markBody =
  markStart >= 0 && ackStart > markStart
    ? files.worker.slice(markStart, ackStart)
    : "";
requireText(
  "raw poison durable publication marker",
  markBody,
  ".mark_published(identity, poison_publisher_id)",
);
for (const forbidden of [
  "move_to_dlq(",
  "acknowledge_decode_failure",
  "mark_acknowledged",
]) {
  forbidText("raw poison published-only retry", markBody, forbidden);
}

const decodedAckStart = files.worker.indexOf("async fn acknowledge_terminal_result(");
const rawAckBody =
  ackStart >= 0 && decodedAckStart > ackStart
    ? files.worker.slice(ackStart, decodedAckStart)
    : "";
const sourceAck = rawAckBody.indexOf(
  "consumer.acknowledge_decode_failure(failure).await",
);
const receiptAck = rawAckBody.indexOf(
  "poison_receipts.mark_acknowledged(identity).await",
);
if (sourceAck < 0 || receiptAck <= sourceAck) {
  failures.push(
    "raw poison source acknowledgement must precede best-effort acknowledged bookkeeping",
  );
}
for (const forbidden of [
  "move_to_dlq(",
  "reserve_and_claim",
  "mark_published",
  "project_consumed",
]) {
  forbidText("raw poison acknowledgement-only path", rawAckBody, forbidden);
}

for (const forbidden of [
  "tenant_id",
  "event_id",
  "ProfilePresentationService",
  "SocialGraphPrivacyReadPort",
  "project_consumed",
]) {
  forbidText("raw poison worker slice", rawFlow, forbidden);
}

if (failures.length > 0) {
  console.error("Iggy consumer poison receipt verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "Iggy consumer poison receipt verification passed: connector-owned migration registration, immutable private source identity, empty exact payload retention, source and UUID collision validation, retry/error classification independence, typed owner delivery, existing-result recovery with DLQ disabled, leased reserve, exact-byte publish, durable published-before-ack ordering, acknowledgement-only recovery, and no fabricated tenant/event/authorization state are locked.",
);
