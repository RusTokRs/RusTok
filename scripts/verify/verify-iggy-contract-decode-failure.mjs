#!/usr/bin/env node

import { readFileSync } from "node:fs";

const files = {
  cargo: readFileSync("crates/rustok-iggy/Cargo.toml", "utf8"),
  lib: readFileSync("crates/rustok-iggy/src/lib.rs", "utf8"),
  consumer: readFileSync(
    "crates/rustok-iggy/src/contract_consumer.rs",
    "utf8",
  ),
  failure: readFileSync(
    "crates/rustok-iggy/src/contract_decode_failure.rs",
    "utf8",
  ),
  crateApi: readFileSync("crates/rustok-iggy/CRATE_API.md", "utf8"),
  plan: readFileSync(
    "crates/rustok-iggy/docs/implementation-plan.md",
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

requireText("rustok-iggy hash dependency", files.cargo, "sha2.workspace = true");
for (const marker of [
  "pub mod contract_decode_failure;",
  "PersistentContractDelivery",
  "ConsumedContractDecodeFailure",
  "ContractDecodeFailureKind",
]) {
  requireText("rustok-iggy public decode-failure API", files.lib, marker);
}

for (const marker of [
  "pub enum PersistentContractDelivery",
  "pub async fn receive_delivery(&self)",
  "self.validate_cursor_metadata(&message.metadata)?;",
  "ContractDecodeFailureKind::Deserialize",
  "ContractDecodeFailureKind::SchemaValidation",
  "pub async fn acknowledge_decode_failure",
  "consumed.connector_metadata()",
  "Persistent contract delivery rejected [{}]",
]) {
  requireText("persistent contract decode-failure cursor", files.consumer, marker);
}

const metadataValidation = files.consumer.indexOf(
  "self.validate_cursor_metadata(&message.metadata)?;",
);
const deserialize = files.consumer.indexOf(
  "self.serializer.deserialize_contract(&raw_payload)",
);
if (metadataValidation < 0 || deserialize <= metadataValidation) {
  failures.push("connector metadata must be validated before contract deserialization");
}

for (const marker of [
  'b"rustok.iggy.contract.decode_failure.delivery_id.v1"',
  'Self::Deserialize => "iggy.contract.decode_invalid"',
  'Self::SchemaValidation => "iggy.contract.schema_invalid"',
  "source_offset: u64",
  "pub const fn offset(&self) -> u64",
  "pub fn connector_metadata(&self)",
  "self.connector_metadata.offset != Some(self.source_offset)",
  "hash_part(&mut hasher, self.stream.as_bytes())",
  "hash_part(&mut hasher, self.topic.as_bytes())",
  "hash_part(&mut hasher, &self.partition.to_be_bytes())",
  "hash_part(&mut hasher, &self.source_offset.to_be_bytes())",
  "hash_part(&mut hasher, &self.raw_payload)",
  "bytes[6] = (bytes[6] & 0x0f) | 0x80",
  "bytes[8] = (bytes[8] & 0x3f) | 0x80",
  ".with_broker_message_id(delivery_id)",
  "delivery_id_is_stable_custom_versioned_and_kind_independent",
  "delivery_id_changes_with_exact_payload_or_source_position",
  "dlq_entry_keeps_exact_bytes_and_stable_connector_identity",
]) {
  requireText("contract decode-failure identity", files.failure, marker);
}
for (const forbidden of [
  "pub stream:",
  "pub topic:",
  "pub partition:",
  "pub connector_metadata:",
  "pub raw_payload:",
  "pub kind:",
  "Uuid::new_v4",
  "SystemTime",
  "DefaultHasher",
  "retry_count.to_be_bytes",
  "stable_error_code().as_bytes",
  "ack_token.as_bytes",
  "message_id.as_bytes",
]) {
  forbidText("contract decode-failure identity", files.failure, forbidden);
}

for (const marker of [
  "does not invent a tenant or domain event id",
  "The first approved owner worker is wired",
  "Choose production confirmation policy",
  "External raw-poison lifecycle harness",
]) {
  requireText("Iggy decode-failure plan", files.plan, marker);
}
for (const marker of [
  "Raw contract decode-failure boundary",
  "does not itself wire Social Graph",
  "Acknowledge a decode failure before a connector-level terminal result exists",
]) {
  requireText("Iggy decode-failure API documentation", files.crateApi, marker);
}

if (failures.length > 0) {
  console.error("Iggy contract decode-failure verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "Iggy contract decode-failure verification passed: metadata-before-decode, immutable exact raw bytes and source coordinates, bounded failure codes, stable UUIDv8 connector identity, explicit post-result acknowledgement, compatibility no-ack behavior, current owner-worker composition, and no invented tenant/event identity are locked.",
);
