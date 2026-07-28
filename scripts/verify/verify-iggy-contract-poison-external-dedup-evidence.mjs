#!/usr/bin/env node

import { readFileSync } from "node:fs";

const contract = JSON.parse(
  readFileSync(
    "crates/rustok-iggy/contracts/evidence/contract-poison-external-iggy-dedup-source.json",
    "utf8",
  ),
);
const test = readFileSync(
  "crates/rustok-iggy/tests/contract_poison_external_iggy_dedup.rs",
  "utf8",
);
const failures = [];

const expectedScenarios = [
  {
    case: "disabled_deduplication_persists_repeated_uuid_twice",
    address_env: "RUSTOK_IGGY_DEDUP_DISABLED_ADDRESS",
    required_reviewed_server_config: {
      "system.message_deduplication.enabled": false,
    },
    publication_sequence: ["A", "A"],
    expected_partition_message_counts: [0, 1, 2],
  },
  {
    case: "enabled_deduplication_suppresses_immediate_repeated_uuid",
    address_env: "RUSTOK_IGGY_DEDUP_ENABLED_ADDRESS",
    required_reviewed_server_config: {
      "system.message_deduplication.enabled": true,
      "system.message_deduplication.max_entries": "at_least_1",
      "system.message_deduplication.expiry": "longer_than_scenario_horizon",
    },
    publication_sequence: ["A", "A"],
    expected_partition_message_counts: [0, 1, 1],
  },
  {
    case: "bounded_deduplication_capacity_eviction_accepts_old_uuid_again",
    address_env: "RUSTOK_IGGY_DEDUP_CAPACITY_ADDRESS",
    required_reviewed_server_config: {
      "system.message_deduplication.enabled": true,
      "system.message_deduplication.max_entries": 1,
      "system.message_deduplication.expiry": "longer_than_scenario_horizon",
    },
    publication_sequence: ["A", "A", "B", "A"],
    expected_partition_message_counts: [0, 1, 1, 2, 3],
  },
  {
    case: "expired_deduplication_entry_accepts_same_uuid_after_bounded_wait",
    address_env: "RUSTOK_IGGY_DEDUP_EXPIRY_ADDRESS",
    additional_env: "RUSTOK_IGGY_DEDUP_EXPIRY_WAIT_MS",
    required_reviewed_server_config: {
      "system.message_deduplication.enabled": true,
      "system.message_deduplication.max_entries": "at_least_1",
      "system.message_deduplication.expiry":
        "shorter_than_RUSTOK_IGGY_DEDUP_EXPIRY_WAIT_MS",
    },
    publication_sequence: ["A", "A", "wait", "A"],
    expected_partition_message_counts: [0, 1, 1, 2],
  },
];
const expectedProductionPaths = [
  "ConsumedContractDecodeFailure::new",
  "ConsumedContractDecodeFailure::to_dlq_entry",
  "IggyTransport::new",
  "IggyTransport::move_to_dlq",
  "IggyTransport::shutdown",
];
const expectedObserverBoundary = {
  purpose: "read_partition_message_count_only",
  allowed_operations: [
    "connect",
    "get_dlq_topic",
    "read_partition_1_messages_count",
    "shutdown",
  ],
  forbidden_operations: [
    "publish",
    "consume_payload",
    "store_offset",
    "modify_server_configuration",
    "delete_stream",
    "mutate_production_receipt",
  ],
};
const expectedForbiddenClaims = [
  "server_configuration_read_back_by_test",
  "physical_exactly_once_proved",
  "receipt_and_broker_transaction_proved",
  "deduplication_window_sufficient_for_production_recovery_proved",
  "bundled_mode_proved",
  "tls_or_auth_failure_proved",
  "multi_replica_proved",
];

function requireText(name, source, marker) {
  if (!source.includes(marker)) {
    failures.push(`${name} is missing required marker: ${marker}`);
  }
}

function forbidText(name, source, marker) {
  if (source.includes(marker)) {
    failures.push(`${name} contains forbidden marker: ${marker}`);
  }
}

function sameValue(actual, expected) {
  return JSON.stringify(actual) === JSON.stringify(expected);
}

function functionBody(source, functionName, nextFunctionName) {
  const start = source.indexOf(`async fn ${functionName}(`);
  const end = source.indexOf(`async fn ${nextFunctionName}(`, start + 1);
  return start >= 0 && end > start ? source.slice(start, end) : "";
}

function requireOrdered(name, source, markers) {
  let previous = -1;
  for (const marker of markers) {
    const position = source.indexOf(marker, previous + 1);
    if (position < 0) {
      failures.push(`${name} is missing ordered marker: ${marker}`);
      return;
    }
    previous = position;
  }
}

if (contract.schema_version !== 1) failures.push("dedup contract schema drift");
if (contract.module !== "iggy") failures.push("dedup contract module drift");
if (contract.packet !== "contract-poison-external-iggy-dedup-source") {
  failures.push("dedup contract packet drift");
}
if (contract.status !== "source_complete_runtime_pending") {
  failures.push("dedup contract must remain runtime pending");
}
if (contract.scope !== "external_real_iggy_message_id_deduplication_behavior") {
  failures.push("dedup contract scope drift");
}
if (contract.test_target !== "contract_poison_external_iggy_dedup") {
  failures.push("dedup test target drift");
}
if (
  contract.source_path !==
  "crates/rustok-iggy/tests/contract_poison_external_iggy_dedup.rs"
) {
  failures.push("dedup source path drift");
}
if (
  contract.verifier !==
  "scripts/verify/verify-iggy-contract-poison-external-dedup-evidence.mjs"
) {
  failures.push("dedup verifier path drift");
}
if (
  !sameValue(contract.shared_optional_environment, [
    "RUSTOK_IGGY_DEDUP_TEST_USERNAME",
    "RUSTOK_IGGY_DEDUP_TEST_PASSWORD",
  ])
) {
  failures.push("dedup credential environment drift");
}
if (!sameValue(contract.scenarios, expectedScenarios)) {
  failures.push("dedup scenario contract drift");
}
if (
  !sameValue(contract.topology, {
    mode: "external",
    protocol: "tcp",
    unique_stream_per_scenario: true,
    domain_partitions: 1,
    replication_factor: 1,
    observed_topic: "dlq",
    observed_partition: 1,
    broker_requirement:
      "four_separately_configured_disposable_or_operator_cleaned_external_iggy_instances",
  })
) {
  failures.push("dedup topology drift");
}
if (!sameValue(contract.production_paths, expectedProductionPaths)) {
  failures.push("dedup production path drift");
}
if (!sameValue(contract.observer_boundary, expectedObserverBoundary)) {
  failures.push("dedup observer boundary drift");
}
if (!sameValue(contract.forbidden_claims, expectedForbiddenClaims)) {
  failures.push("dedup forbidden claim drift");
}
if (contract.execution_status !== "not_run") {
  failures.push("dedup contract must not claim execution");
}

for (const marker of [
  '#![cfg(feature = "iggy")]',
  "use iggy::prelude::{Client, Identifier, IggyClient, TopicClient};",
  'const DISABLED_ADDRESS_ENV: &str = "RUSTOK_IGGY_DEDUP_DISABLED_ADDRESS";',
  'const ENABLED_ADDRESS_ENV: &str = "RUSTOK_IGGY_DEDUP_ENABLED_ADDRESS";',
  'const CAPACITY_ADDRESS_ENV: &str = "RUSTOK_IGGY_DEDUP_CAPACITY_ADDRESS";',
  'const EXPIRY_ADDRESS_ENV: &str = "RUSTOK_IGGY_DEDUP_EXPIRY_ADDRESS";',
  'const USERNAME_ENV: &str = "RUSTOK_IGGY_DEDUP_TEST_USERNAME";',
  'const PASSWORD_ENV: &str = "RUSTOK_IGGY_DEDUP_TEST_PASSWORD";',
  'const EXPIRY_WAIT_ENV: &str = "RUSTOK_IGGY_DEDUP_EXPIRY_WAIT_MS";',
  "const MIN_EXPIRY_WAIT_MS: u64 = 100;",
  "const MAX_EXPIRY_WAIT_MS: u64 = 300_000;",
  "IggyMode::External",
  'protocol: "tcp".to_string()',
  "domain_partitions: 1",
  "replication_factor: 1",
  "ConsumedContractDecodeFailure::new(",
  ".to_dlq_entry(1)",
  ".get_topic(&self.stream_id, &self.topic_id)",
  "partition.id == 1",
  "partition.messages_count",
  "self.client.shutdown().await?;",
  "self.transport.shutdown().await?;",
  "tokio::time::sleep(wait).await;",
  "(MIN_EXPIRY_WAIT_MS..=MAX_EXPIRY_WAIT_MS).contains(&millis)",
]) {
  requireText("external Iggy dedup test", test, marker);
}

const disabled = functionBody(
  test,
  "disabled_deduplication_persists_repeated_uuid_twice",
  "enabled_deduplication_suppresses_immediate_repeated_uuid",
);
requireOrdered("disabled dedup scenario", disabled, [
  "assert_message_count(0)",
  "move_to_dlq(entry.clone())",
  "assert_message_count(1)",
  "move_to_dlq(entry)",
  "assert_message_count(2)",
]);

const enabled = functionBody(
  test,
  "enabled_deduplication_suppresses_immediate_repeated_uuid",
  "bounded_deduplication_capacity_eviction_accepts_old_uuid_again",
);
requireOrdered("enabled dedup scenario", enabled, [
  "assert_message_count(0)",
  "move_to_dlq(entry.clone())",
  "assert_message_count(1)",
  "move_to_dlq(entry)",
  "assert_message_count(1)",
]);

const capacity = functionBody(
  test,
  "bounded_deduplication_capacity_eviction_accepts_old_uuid_again",
  "expired_deduplication_entry_accepts_same_uuid_after_bounded_wait",
);
for (const marker of [
  "assert_ne!(first.broker_message_id(), second.broker_message_id())",
  "assert_message_count(0)",
  "move_to_dlq(first.clone())",
  "assert_message_count(1)",
  "move_to_dlq(second)",
  "assert_message_count(2)",
  "move_to_dlq(first)",
  "assert_message_count(3)",
]) {
  requireText("capacity eviction scenario", capacity, marker);
}
const capacityFirstPublishCount = [
  ...capacity.matchAll(/move_to_dlq\(first\.clone\(\)\)/gu),
].length;
if (capacityFirstPublishCount !== 2) {
  failures.push(
    `capacity eviction scenario must immediately publish A twice before B; found ${capacityFirstPublishCount}`,
  );
}

const expiryStart = test.indexOf(
  "async fn expired_deduplication_entry_accepts_same_uuid_after_bounded_wait(",
);
const harnessStart = test.indexOf("struct ExternalDedupHarness", expiryStart);
const expiry =
  expiryStart >= 0 && harnessStart > expiryStart
    ? test.slice(expiryStart, harnessStart)
    : "";
requireOrdered("expiry dedup scenario", expiry, [
  "assert_message_count(0)",
  "move_to_dlq(entry.clone())",
  "assert_message_count(1)",
  "move_to_dlq(entry.clone())",
  "assert_message_count(1)",
  "tokio::time::sleep(wait).await;",
  "move_to_dlq(entry)",
  "assert_message_count(2)",
]);

const moveToDlqCount = [...test.matchAll(/\.move_to_dlq\(/gu)].length;
if (moveToDlqCount !== 11) {
  failures.push(`dedup evidence must contain exactly 11 production publications; found ${moveToDlqCount}`);
}
const sleepCount = [...test.matchAll(/tokio::time::sleep\(/gu)].length;
if (sleepCount !== 1) {
  failures.push(`dedup evidence must contain one bounded expiry sleep; found ${sleepCount}`);
}

const observerStart = test.indexOf("async fn message_count(&self)");
const assertCountStart = test.indexOf("async fn assert_message_count", observerStart);
const observerBody =
  observerStart >= 0 && assertCountStart > observerStart
    ? test.slice(observerStart, assertCountStart)
    : "";
for (const marker of [
  ".get_topic(&self.stream_id, &self.topic_id)",
  "partition.id == 1",
  "partition.messages_count",
]) {
  requireText("dedup read-only observer", observerBody, marker);
}
for (const forbidden of [
  ".producer(",
  ".send(",
  ".consumer_group(",
  ".store_offset(",
  "IggyMessage",
  "payload",
  "move_to_dlq",
]) {
  forbidText("dedup read-only observer", observerBody, forbidden);
}

for (const forbidden of [
  "127.0.0.1:8090",
  'username: "iggy"',
  'password: "iggy"',
  "ExternalConnector",
  "PublishRequest",
  "IggyMessage::builder",
  ".producer(",
  ".send(",
  ".consumer_group(",
  ".store_offset(",
  "ConsumerPoisonReceiptStore",
  "mark_published",
  "mark_acknowledged",
  "delete_stream",
  "DATABASE_URL",
  "println!(connection_string",
  "tracing::",
]) {
  forbidText("external Iggy dedup test", test, forbidden);
}

if (failures.length > 0) {
  console.error("External Iggy dedup evidence verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "External Iggy dedup source evidence verified: disabled, immediate suppression, capacity eviction, expiry, production DLQ publication, read-only partition counts, reviewed external config boundaries, and bounded unexecuted claims are locked.",
);
