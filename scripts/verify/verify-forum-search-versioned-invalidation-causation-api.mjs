#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();

function filePath(relativePath) {
  return path.join(root, relativePath);
}

function read(relativePath) {
  return fs.readFileSync(filePath(relativePath), "utf8");
}

function readJson(relativePath) {
  return JSON.parse(read(relativePath));
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) {
    throw new Error(`${label} is missing required marker: ${marker}`);
  }
}

function forbidMarker(source, marker, label) {
  if (source.includes(marker)) {
    throw new Error(`${label} contains forbidden marker: ${marker}`);
  }
}

const eventsContractPath = "crates/rustok-events/src/contract.rs";
const eventsLibPath = "crates/rustok-events/src/lib.rs";
const eventsApiPath = "crates/rustok-events/CRATE_API.md";
const digestPath = "crates/rustok-events/contracts/event-contract-digests.json";
const outboxTransactionalPath = "crates/rustok-outbox/src/transactional.rs";
const outboxTransportPath = "crates/rustok-outbox/src/transport.rs";
const outboxApiPath = "crates/rustok-outbox/CRATE_API.md";
const forumPublisherPath = "crates/rustok-forum/src/services/projection_invalidation.rs";
const machineContractPath =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-causation-api.json";
const publisherContractPath =
  "crates/rustok-forum/contracts/forum-search-versioned-invalidation-publisher.json";
const ownerNotePath =
  "crates/rustok-forum/docs/forum-23b2g2b3b1-causation-publication-api.md";

const sources = new Map([
  [eventsContractPath, read(eventsContractPath)],
  [eventsLibPath, read(eventsLibPath)],
  [eventsApiPath, read(eventsApiPath)],
  [outboxTransactionalPath, read(outboxTransactionalPath)],
  [outboxTransportPath, read(outboxTransportPath)],
  [outboxApiPath, read(outboxApiPath)],
  [forumPublisherPath, read(forumPublisherPath)],
  [ownerNotePath, read(ownerNotePath)],
]);
const digest = readJson(digestPath);
const contract = readJson(machineContractPath);
const publisherDelivered = fs.existsSync(filePath(publisherContractPath));
const publisherContract = publisherDelivered ? readJson(publisherContractPath) : null;

for (const [marker, sourcePath] of [
  ["pub fn new_caused_by<E>(", eventsContractPath],
  ["Self::new_with_causation(tenant_id, actor_id, Some(causation_id), event)", eventsContractPath],
  ["pub fn causation_id(&self) -> Option<Uuid>", eventsContractPath],
  ["EventValidationError::NilUuid(\"causation_id\")", eventsContractPath],
  ["caused_contract_envelope_retains_exact_causation_identity", eventsContractPath],
  ["caused_contract_envelope_rejects_nil_causation_identity", eventsContractPath],
  ["ContractEventEnvelope::new_caused_by", eventsApiPath],
  ["publish_contract_direct_in_tx_with_causation_and_envelope_id", outboxTransactionalPath],
  ["publish_contract_in_tx_with_causation<C, E>", outboxTransactionalPath],
  ["publish_contract_in_tx_with_causation_and_envelope_id<C, E>", outboxTransactionalPath],
  ["ContractEventEnvelope::new_caused_by", outboxTransactionalPath],
  ["OutboxTransport::write_contract_envelope_in_tx(txn, envelope)", outboxTransactionalPath],
  ["pub(crate) async fn write_contract_envelope_in_tx<C>", outboxTransportPath],
  ["publish_contract_in_tx_with_causation", outboxApiPath],
  ["FORUM-23B2G2B3B1", ownerNotePath],
  ["FORUM-23B2G2B3B2", ownerNotePath],
]) {
  requireMarker(sources.get(sourcePath), marker, sourcePath);
}

if (contract.task !== "FORUM-23B2G2B3B1") {
  throw new Error(`${machineContractPath} has the wrong task`);
}
if (contract.status !== "source_complete_family_publisher_pending") {
  throw new Error(`${machineContractPath} has the wrong historical status`);
}
if (contract.envelope_api?.constructor !== "ContractEventEnvelope::new_caused_by"
    || contract.envelope_api?.getter !== "ContractEventEnvelope::causation_id"
    || contract.envelope_api?.wire_fields_added !== false
    || contract.envelope_api?.json_schema_changed !== false
    || contract.envelope_api?.event_digest_changed !== false) {
  throw new Error(`${machineContractPath} causation API boundary drift`);
}
if (
  contract.publication_api?.canonical_writer !==
  "OutboxTransport::write_contract_envelope_in_tx"
  || contract.publication_api?.requires_live_owner_transaction !== true
  || contract.publication_api?.creates_second_transport_path !== false
) {
  throw new Error(`${machineContractPath} outbox publication boundary drift`);
}
if (contract.compatibility?.["ContractEventEnvelope::new_retained"] !== true
    || contract.compatibility?.sealed_event_family_added !== false
    || contract.compatibility?.event_digest_artifact_changed !== false
    || contract.follow_up?.task !== "FORUM-23B2G2B3B2") {
  throw new Error(`${machineContractPath} historical slice facts drift`);
}

const prePublisherDigest = {
  format_version: 1,
  registry: "sha256:18fdee49d915a22ed3dd709ec6cc1826d6d47a59ddfe659fbd07ede7f6cd3d07",
  root_event: "sha256:2bc388a237ff1fcbe327c340633815a64c84c799afef5a0012f458752d6deb87",
  root_envelope: "sha256:cfb55b9ac1fbebdc27658e035c00a98468c947b4830f8603c4258457849db42d",
  contract_payload: "sha256:5f1f9577bc9429b76bbfe5420d1bd71249efd6aea702ec0a103e4a402702cd02",
  contract_envelope: "sha256:b29a8c7809045e14f1233db4e2f9dba9cedf96352df3ea9e87ca1e86f6a59eb8",
};

if (!publisherDelivered) {
  for (const [marker, sourcePath] of [
    ["ForumSearchProjectionEvent", eventsContractPath],
    ["ForumSearchProjectionEvent", eventsLibPath],
    ["forum_search_projection", eventsContractPath],
    ["ForumSearchProjectionEvent", forumPublisherPath],
  ]) {
    forbidMarker(sources.get(sourcePath), marker, sourcePath);
  }
  if (JSON.stringify(digest) !== JSON.stringify(prePublisherDigest)) {
    throw new Error(`${digestPath} changed before the named publisher slice`);
  }
} else {
  if (publisherContract.task !== "FORUM-23B2G2B3B2"
      || publisherContract.status !== "source_complete_consumer_pending") {
    throw new Error(`${publisherContractPath} does not prove the downstream publisher slice`);
  }
  for (const [marker, sourcePath] of [
    ["ForumSearchProjectionEvent", eventsContractPath],
    ["ForumSearchProjectionEvent", eventsLibPath],
    ["forum_search_projection", eventsContractPath],
    ["ForumSearchProjectionEvent", forumPublisherPath],
    ["publish_contract_in_tx_with_causation", forumPublisherPath],
  ]) {
    requireMarker(sources.get(sourcePath), marker, sourcePath);
  }
  const expected = {
    format_version: 1,
    ...publisherContract.digest_generation.new_values,
  };
  if (JSON.stringify(digest) !== JSON.stringify(expected)) {
    throw new Error(`${digestPath} does not match the delivered publisher contract`);
  }
}

console.log(
  "FORUM-23B2G2B3B1 caused contract publication API source contract verified",
);
