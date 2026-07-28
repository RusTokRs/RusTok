#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-social-graph/contracts/evidence/index-raw-poison-publish-mark-ambiguity-source.json";
const testPath =
  "crates/rustok-social-graph/tests/index_raw_poison_publish_mark_ambiguity.rs";
const cargoPath = "crates/rustok-social-graph/Cargo.toml";
const receiptPath = "crates/rustok-iggy-connector/src/consumer_poison_receipt.rs";
const transportPath = "crates/rustok-iggy/src/transport.rs";
const workerPath = "apps/server/src/services/social_graph_index_worker.rs";
const expectedVerifier =
  "scripts/verify/verify-social-graph-index-raw-poison-publish-mark-ambiguity.mjs";
const expectedDocumentation =
  "crates/rustok-social-graph/docs/index-raw-poison-publish-mark-ambiguity-evidence.md";
const expectedProfilesCheckpoint =
  "crates/rustok-profiles/docs/poison-publish-mark-ambiguity-checkpoint.md";
const expectedScenarios = [
  {
    case: "dedup_enabled_closes_publish_mark_ambiguity_without_physical_duplicate",
    address_environment:
      "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_DEDUP_ENABLED_ADDRESS",
    required_observed_dlq_counts: [0, 1, 1],
  },
  {
    case: "dedup_disabled_exposes_publish_mark_ambiguity_as_physical_duplicate",
    address_environment:
      "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_DEDUP_DISABLED_ADDRESS",
    required_observed_dlq_counts: [0, 1, 2],
  },
];
const expectedProductionOrder = [
  "reserve_and_claim",
  "transport.move_to_dlq",
  "mark_raw_poison_published",
  "acknowledge_decode_failure",
  "mark_acknowledged",
];

const contract = JSON.parse(readFileSync(resolve(repoRoot, contractPath), "utf8"));
const test = readFileSync(resolve(repoRoot, testPath), "utf8");
const cargo = readFileSync(resolve(repoRoot, cargoPath), "utf8");
const receipt = readFileSync(resolve(repoRoot, receiptPath), "utf8");
const transport = readFileSync(resolve(repoRoot, transportPath), "utf8");
const worker = readFileSync(resolve(repoRoot, workerPath), "utf8");
const failures = [];

function fail(message) {
  failures.push(message);
}

function sameValue(actual, expected) {
  return JSON.stringify(actual) === JSON.stringify(expected);
}

function requireText(name, source, marker) {
  if (!source.includes(marker)) fail(`${name} is missing required marker: ${marker}`);
}

function forbidText(name, source, marker) {
  if (source.includes(marker)) fail(`${name} contains forbidden marker: ${marker}`);
}

function countText(source, marker) {
  return source.split(marker).length - 1;
}

function requireOrder(name, source, markers) {
  let cursor = -1;
  for (const marker of markers) {
    const next = source.indexOf(marker, cursor + 1);
    if (next < 0) {
      fail(`${name} is missing ordered marker: ${marker}`);
      return;
    }
    if (next <= cursor) {
      fail(`${name} marker order drifted at: ${marker}`);
      return;
    }
    cursor = next;
  }
}

if (
  contract.schema_version !== 1 ||
  contract.module !== "social-graph" ||
  contract.packet !== "index-raw-poison-publish-mark-ambiguity-source" ||
  contract.status !== "source_complete_runtime_pending" ||
  contract.test_target !== "index_raw_poison_publish_mark_ambiguity" ||
  contract.feature !== "index-consumer" ||
  contract.execution_status !== "not_run"
) {
  fail("publish/mark ambiguity contract identity or runtime-pending status drift");
}

if (
  contract.database_environment !==
    "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_DATABASE_URL" ||
  !sameValue(contract.shared_optional_environment, [
    "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_IGGY_USERNAME",
    "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_IGGY_PASSWORD",
  ])
) {
  fail("publish/mark ambiguity environment boundary drift");
}

const retainedScenarios = contract.scenarios?.map((scenario) => ({
  case: scenario.case,
  address_environment: scenario.address_environment,
  required_observed_dlq_counts: scenario.required_observed_dlq_counts,
}));
if (!sameValue(retainedScenarios, expectedScenarios)) {
  fail("publish/mark ambiguity scenario or physical count contract drift");
}

if (
  contract.lease_boundary?.duration_seconds !== 1 ||
  contract.lease_boundary?.wait_milliseconds !== 1500 ||
  contract.lease_boundary?.direct_receipt_sql_mutation !== false ||
  contract.lease_boundary?.clock_source !== "postgresql_current_timestamp"
) {
  fail("publish/mark ambiguity lease boundary drift");
}

if (
  contract.broker_observer?.implementation !==
    "read_only_iggy_sdk_topic_metadata" ||
  contract.broker_observer?.allowed_operation !==
    "get_topic_partition_messages_count" ||
  contract.broker_observer?.direct_sdk_producer !== false ||
  contract.broker_observer?.unique_stream_per_case !== true ||
  contract.broker_observer?.domain_partitions !== 1 ||
  contract.broker_observer?.replication_factor !== 1
) {
  fail("publish/mark ambiguity read-only broker observer boundary drift");
}

if (!sameValue(contract.required_production_order, expectedProductionOrder)) {
  fail("publish/mark ambiguity production order contract drift");
}
if (
  contract.verifier !== expectedVerifier ||
  contract.documentation !== expectedDocumentation ||
  contract.profiles_checkpoint !== expectedProfilesCheckpoint
) {
  fail("publish/mark ambiguity verifier or documentation path drift");
}

const expectedSources = [
  testPath,
  cargoPath,
  receiptPath,
  transportPath,
  workerPath,
];
if (!sameValue(contract.source_files, expectedSources)) {
  fail("publish/mark ambiguity source file allowlist drift");
}

for (const marker of [
  "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_DATABASE_URL",
  "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_DEDUP_ENABLED_ADDRESS",
  "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_DEDUP_DISABLED_ADDRESS",
  "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_IGGY_USERNAME",
  "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_AMBIGUITY_IGGY_PASSWORD",
  "const PUBLISH_LEASE: Duration = Duration::from_secs(1);",
  "const LEASE_RECLAIM_WAIT: Duration = Duration::from_millis(1_500);",
  "dedup_enabled_closes_publish_mark_ambiguity_without_physical_duplicate",
  "dedup_disabled_exposes_publish_mark_ambiguity_as_physical_duplicate",
  "ensure_distinct_mode_addresses()?",
  "ConsumerPoisonPublishClaim::Busy",
  "tokio::time::sleep(LEASE_RECLAIM_WAIT).await;",
  "assert_eq!(redelivered.offset(), first_offset);",
  "assert_eq!(redelivered.delivery_id(), first_delivery_id);",
  "assert_eq!(redelivered.raw_payload(), first_payload.as_slice());",
  "Err(ConsumerPoisonReceiptError::ClaimLost)",
  "assert_eq!(retry_entry.broker_message_id(), Some(first_broker_message_id));",
  "assert_message_count(&observer, &stream, expected_retry_count).await?;",
  "ConsumerPoisonReceiptState::Published",
  "acknowledge_decode_failure(&redelivered)",
  "store.mark_acknowledged(&identity).await?;",
  "ConsumerPoisonReceiptState::Acknowledged",
  "assert!(next_failure.offset() > first_offset);",
  ".get_topic(&stream_id, &topic_id)",
  "partition.messages_count",
  ".max_connections(1)",
  "DROP SCHEMA IF EXISTS",
]) {
  requireText("publish/mark ambiguity source harness", test, marker);
}

if (countText(test, ".move_to_dlq(") !== 2) {
  fail("publish/mark ambiguity harness must contain exactly two production DLQ publishes");
}
if (countText(test, ".reserve_and_claim(") !== 3) {
  fail("publish/mark ambiguity harness must contain first, busy, and recovery claims");
}
if (countText(test, "assert_message_count(") < 4) {
  fail("publish/mark ambiguity harness has insufficient physical count observations");
}

requireOrder("publish/mark ambiguity recovery helper", test, [
  "ConsumerPoisonPublishClaim::Claimed",
  "first_transport.move_to_dlq(first_entry).await?;",
  "ConsumerPoisonPublishClaim::Busy",
  "first_transport.shutdown().await?;",
  "tokio::time::sleep(LEASE_RECLAIM_WAIT).await;",
  "let redelivered = receive_decode_failure(&recovery_consumer).await?;",
  "Err(ConsumerPoisonReceiptError::ClaimLost)",
  "recovery_transport.move_to_dlq(retry_entry).await?;",
  "mark_published(&identity, recovery_publisher)",
  "acknowledge_decode_failure(&redelivered)",
  "store.mark_acknowledged(&identity).await?;",
]);

for (const marker of [
  "DATABASE_URL",
  "localhost",
  "127.0.0.1",
  "UPDATE iggy_consumer_poison_receipts",
  "lease_expires_at =",
  "Statement::from_sql",
  ".send_messages(",
  ".send_message(",
  "delete_stream",
  "remove_stream",
]) {
  forbidText("publish/mark ambiguity source harness", test, marker);
}

requireText("Social Graph Cargo dev dependencies", cargo, "iggy.workspace = true");
requireText(
  "Social Graph Cargo dev dependencies",
  cargo,
  'rustok-iggy-connector = { workspace = true, features = ["migrations"] }',
);

for (const marker of [
  "must be a whole number of seconds",
  "lease_expires_at <= CURRENT_TIMESTAMP",
  "ConsumerPoisonReceiptError::ClaimLost",
  "WHERE publisher_id =",
  "state = 'publishing'",
]) {
  requireText("consumer poison receipt store", receipt, marker);
}

for (const marker of [
  "if entry.broker_message_id().is_some()",
  "IggyDlqPublisher::connect",
  ".publish(&entry)",
]) {
  requireText("production Iggy transport", transport, marker);
}

requireOrder("production Social Graph raw poison worker", worker, [
  ".reserve_and_claim(",
  "transport.move_to_dlq(failure.to_dlq_entry(1)).await",
  "mark_raw_poison_published(",
  "consumer.acknowledge_decode_failure(failure).await",
  "poison_receipts.mark_acknowledged(identity).await",
]);
for (const marker of [
  "Raw poison bytes were published but durable published state failed; retrying persistence only",
  "source offset committed but receipt acknowledgement bookkeeping failed",
  "redelivery retries acknowledgement only",
]) {
  requireText("production Social Graph raw poison worker", worker, marker);
}

const requiredNonClaims = new Set([
  "postgresql_iggy_transaction",
  "physical_exactly_once_without_deduplication",
  "production_dedup_window_sufficiency",
  "dedup_configuration_readback",
  "bundled_iggy",
  "tls_auth_failover",
  "multi_replica_ownership",
  "profiles_authorization",
]);
for (const claim of contract.non_claims ?? []) requiredNonClaims.delete(claim);
if (requiredNonClaims.size > 0) {
  fail(`publish/mark ambiguity non-claims are incomplete: ${[...requiredNonClaims].join(", ")}`);
}

if (failures.length > 0) {
  console.error("Social Graph publish/mark ambiguity source verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Social Graph publish/mark ambiguity source verified: PostgreSQL lease recovery, deterministic retry identity, dedup-enabled 0→1→1 behavior, dedup-disabled 0→1→2 behavior, production worker ordering, and bounded non-claims are locked; runtime execution remains pending.",
);
