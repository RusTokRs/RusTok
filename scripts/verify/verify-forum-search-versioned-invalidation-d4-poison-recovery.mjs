#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const files = {
  parent: "crates/rustok-forum/contracts/forum-search-versioned-invalidation-runtime-evidence.json",
  contract:
    "crates/rustok-forum/contracts/forum-search-versioned-invalidation-d4-poison-recovery-source-proof.json",
  handoff:
    "crates/rustok-forum/docs/forum-23b2g2b3d4-versioned-invalidation-poison-recovery.md",
  poisonTests: "apps/server/tests/forum_search_poison_protocol.rs",
  worker: "apps/server/src/services/forum_search_contract_consumer.rs",
  decode: "crates/rustok-iggy/src/contract_decode_failure.rs",
  dlq: "crates/rustok-iggy/src/dlq.rs",
  receipts: "crates/rustok-iggy-connector/src/consumer_poison_receipt.rs",
};

const errors = [];

function read(relativePath) {
  const absolutePath = path.join(root, relativePath);
  if (!fs.existsSync(absolutePath)) {
    errors.push(`missing required file: ${relativePath}`);
    return "";
  }
  return fs.readFileSync(absolutePath, "utf8");
}

function parseJson(relativePath, content) {
  try {
    return JSON.parse(content);
  } catch (error) {
    errors.push(`invalid JSON in ${relativePath}: ${error.message}`);
    return {};
  }
}

function requireMarkers(label, content, markers) {
  for (const marker of markers) {
    if (!content.includes(marker)) {
      errors.push(`${label} is missing marker: ${marker}`);
    }
  }
}

function forbidMarkers(label, content, markers) {
  for (const marker of markers) {
    if (content.includes(marker)) {
      errors.push(`${label} contains forbidden marker: ${marker}`);
    }
  }
}

function requireOrder(label, content, markers) {
  let cursor = -1;
  for (const marker of markers) {
    const next = content.indexOf(marker, cursor + 1);
    if (next === -1) {
      errors.push(`${label} is missing ordered marker: ${marker}`);
      return;
    }
    cursor = next;
  }
}

const parentText = read(files.parent);
const contractText = read(files.contract);
const handoff = read(files.handoff);
const poisonTests = read(files.poisonTests);
const worker = read(files.worker);
const decode = read(files.decode);
const dlq = read(files.dlq);
const receipts = read(files.receipts);

const parent = parseJson(files.parent, parentText);
const contract = parseJson(files.contract, contractText);

if (contract.task !== "FORUM-23B2G2B3D4") {
  errors.push("D4 contract task must be FORUM-23B2G2B3D4");
}
if (contract.predecessor !== "FORUM-23B2G2B3D3") {
  errors.push("D4 contract predecessor must be merged D3");
}
if (contract.status !== "source_ready_maintainer_execution_pending") {
  errors.push("D4 contract must remain source_ready_maintainer_execution_pending");
}
if (
  !Array.isArray(parent.source_ready_subproofs) ||
  !parent.source_ready_subproofs.some(
    (entry) =>
      entry.task === "FORUM-23B2G2B3D4" &&
      entry.contract === files.contract,
  )
) {
  errors.push("parent D0 contract does not register the D4 source proof");
}
if (
  !parent.source_ready_subproofs.some(
    (entry) => entry.task === "FORUM-23B2G2B3D3",
  )
) {
  errors.push("parent D0 contract lost the merged D3 acknowledgement proof");
}

requireMarkers("D4 handoff", handoff, [
  "FORUM-23B2G2B3D3",
  "reserve_and_claim",
  "mark_published",
  "acknowledge exact source position",
  "mark_acknowledged",
  "AlreadyPublished",
  "forum.search_projection.contract_inbox_identity_conflict",
  "confirmation ambiguity",
  "source_ready_maintainer_execution_pending",
]);

requireMarkers("poison protocol tests", poisonTests, [
  "rustok_iggy_connector::migrations::migrations()",
  "ConsumerPoisonReceiptStore",
  "ConsumerPoisonPublishClaim::Claimed",
  "ConsumerPoisonPublishClaim::AlreadyPublished",
  "ConsumerPoisonReceiptState::Published",
  "ConsumerPoisonReceiptState::Acknowledged",
  "release_claim",
  "mark_published",
  "mark_acknowledged",
  "raw_poison_redelivery_is_ack_only_after_durable_publication",
  "semantic_poison_reuses_the_same_durable_dlq_protocol",
  "failed_dlq_publication_releases_the_claim_for_restart",
  "forum.search_projection.contract_inbox_identity_conflict",
  "with_broker_message_id",
]);

requireOrder("source proof durable publication", poisonTests, [
  "reserve_and_claim(",
  "publisher.publish(entry)",
  "mark_published(identity, publisher_id)",
]);
requireOrder("production poison publication", worker, [
  ".reserve_and_claim(",
  "transport.move_to_dlq(entry.clone())",
  "mark_poison_published(",
]);
requireOrder("production raw poison terminalization", worker, [
  "establish_poison_result(",
  "acknowledge_decode_failure_with_receipt(",
]);
requireOrder("production semantic poison terminalization", worker, [
  "establish_poison_result(",
  "acknowledge_event_with_receipt(",
]);

requireMarkers("production poison worker", worker, [
  "ConsumerPoisonPublishClaim::AlreadyPublished",
  "ConsumerPoisonPublishClaim::AlreadyAcknowledged",
  "if !config.dlq_enabled && !continuing_receipt",
  "release_claim(identity, poison_publisher_id)",
  "mark_acknowledged(identity)",
  "broker offset remains uncommitted",
]);

requireMarkers("deterministic decode identity", decode, [
  "CONTRACT_DECODE_FAILURE_ID_DOMAIN",
  "Failure kind, retry count, time, process identity",
  "with_broker_message_id(delivery_id)",
]);
requireMarkers("DLQ entry", dlq, [
  "broker_message_id",
  "publish_event_id",
  "entry.broker_message_id.unwrap_or(entry.event_id)",
]);
requireMarkers("durable receipt store", receipts, [
  "ConsumerPoisonReceiptState::Reserved",
  "ConsumerPoisonReceiptState::Publishing",
  "ConsumerPoisonReceiptState::Published",
  "ConsumerPoisonReceiptState::Acknowledged",
  "reserve_and_claim",
  "release_claim",
  "mark_published",
  "mark_acknowledged",
  "IdentityConflict",
]);

forbidMarkers("D4 source proof", `${contractText}\n${handoff}\n${poisonTests}`, [
  "second_search_projection_inbox",
  "exactly_once_claim",
  "external_iggy_executed",
  "tests_passed",
  "LINK-FORUM-03 closed",
]);

if (errors.length > 0) {
  console.error("Forum Search D4 poison recovery verification failed:");
  for (const error of errors) {
    console.error(`- ${error}`);
  }
  process.exit(1);
}

console.log(
  "Forum Search D4 poison recovery source proof is internally consistent.",
);
