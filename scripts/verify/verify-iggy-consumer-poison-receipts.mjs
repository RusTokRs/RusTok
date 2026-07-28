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
  "lease_expires_at <= CURRENT_TIMESTAMP",
  "retained.stable_error_code",
  "retained.first_delivery_attempt_count",
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
]) {
  requireText("raw decode-failure bridge facts", files.decodeFailure, marker);
}

const insert = files.receipts.indexOf("insert_receipt_sql(backend)");
const select = files.receipts.indexOf("select_receipt_sql(backend, true)", insert);
const claim = files.receipts.indexOf("claim_receipt_sql(backend)", select);
if (insert < 0 || select <= insert || claim <= select) {
  failures.push(
    "receipt reservation must persist, reload/validate, and only then claim publication",
  );
}

if (failures.length > 0) {
  console.error("Iggy consumer poison receipt verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "Iggy consumer poison receipt verification passed: connector-owned migration registration, immutable private source identity, exact-byte conflict validation, retry/error classification independence, retained first diagnostics, leased reserve/publish/ack states, bounded stable errors, and no tenant/event/authorization or implicit broker side effects are locked.",
);
