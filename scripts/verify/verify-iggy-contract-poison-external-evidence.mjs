#!/usr/bin/env node

import { readFileSync } from "node:fs";

const contract = JSON.parse(
  readFileSync(
    "crates/rustok-iggy/contracts/evidence/contract-poison-external-iggy-source.json",
    "utf8",
  ),
);
const test = readFileSync(
  "crates/rustok-iggy/tests/contract_poison_external_iggy.rs",
  "utf8",
);
const failures = [];

const expectedSequence = [
  "open_source_and_dlq_consumer_groups_before_fixture_publish",
  "publish_two_distinct_nonempty_malformed_payloads_to_one_domain_partition",
  "receive_first_payload_through_persistent_contract_consumer_without_ack",
  "classify_first_payload_as_decode_invalid_and_retain_exact_bytes_offset_ack_token",
  "publish_first_failure_through_iggy_transport_move_to_dlq",
  "receive_and_ack_exact_first_dlq_payload",
  "drop_source_cursor_without_ack",
  "reopen_same_source_group_and_receive_same_offset_payload_and_delivery_uuid",
  "explicitly_acknowledge_redelivered_failure",
  "receive_second_payload_at_a_greater_offset",
  "publish_and_verify_exact_second_dlq_payload",
  "explicitly_acknowledge_second_failure",
  "shutdown_fixture_connector_and_transport",
];
const expectedProductionPaths = [
  "IggyTransport::new",
  "IggyTransport::open_persistent_contract_consumer_group",
  "PersistentContractConsumerGroup::receive_delivery",
  "ConsumedContractDecodeFailure::to_dlq_entry",
  "IggyTransport::move_to_dlq",
  "PersistentContractConsumerGroup::acknowledge_decode_failure",
  "ExternalConnector::open_consumer_group",
];
const expectedForbiddenClaims = [
  "database_receipt_ordering_proved",
  "deterministic_iggy_header_runtime_proved",
  "broker_deduplication_runtime_proved",
  "physical_exactly_once_proved",
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

function requireOrdered(name, source, markers) {
  let previous = -1;
  for (const marker of markers) {
    const position = source.indexOf(marker, previous + 1);
    if (position < 0) {
      failures.push(`${name} is missing ordered marker: ${marker}`);
      return;
    }
    if (position <= previous) {
      failures.push(`${name} has invalid ordering at marker: ${marker}`);
      return;
    }
    previous = position;
  }
}

if (contract.schema_version !== 1) failures.push("source contract schema_version drift");
if (contract.module !== "iggy") failures.push("source contract module drift");
if (contract.packet !== "contract-poison-external-iggy-source") {
  failures.push("source contract packet drift");
}
if (contract.status !== "source_complete_runtime_pending") {
  failures.push("source contract must remain runtime-pending until execution");
}
if (contract.scope !== "external_real_iggy_raw_poison_cursor_lifecycle") {
  failures.push("source contract scope drift");
}
if (contract.test_target !== "contract_poison_external_iggy") {
  failures.push("source contract test target drift");
}
if (
  contract.test_case !==
  "malformed_delivery_redelivers_until_explicit_ack_and_dlq_keeps_exact_bytes"
) {
  failures.push("source contract test case drift");
}
if (
  contract.source_path !==
  "crates/rustok-iggy/tests/contract_poison_external_iggy.rs"
) {
  failures.push("source contract path drift");
}
if (
  contract.verifier !==
  "scripts/verify/verify-iggy-contract-poison-external-evidence.mjs"
) {
  failures.push("source contract verifier drift");
}
if (!sameValue(contract.required_environment, ["RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS"])) {
  failures.push("source contract required environment drift");
}
if (
  !sameValue(contract.optional_environment, [
    "RUSTOK_IGGY_EXTERNAL_TEST_USERNAME",
    "RUSTOK_IGGY_EXTERNAL_TEST_PASSWORD",
  ])
) {
  failures.push("source contract optional environment drift");
}
if (
  !sameValue(contract.topology, {
    mode: "external",
    protocol: "tcp",
    unique_stream_per_run: true,
    domain_partitions: 1,
    replication_factor: 1,
    topics: ["domain", "dlq"],
    broker_requirement: "disposable_or_operator_cleaned_external_iggy",
  })
) {
  failures.push("source contract topology drift");
}
if (!sameValue(contract.required_sequence, expectedSequence)) {
  failures.push("source contract required sequence drift");
}
if (!sameValue(contract.production_paths, expectedProductionPaths)) {
  failures.push("source contract production path drift");
}
if (
  contract.fixture_only_path !==
  "ExternalConnector::publish(PublishRequest) injects arbitrary malformed broker bytes"
) {
  failures.push("source contract fixture boundary drift");
}
if (!sameValue(contract.forbidden_claims, expectedForbiddenClaims)) {
  failures.push("source contract forbidden claim drift");
}
if (contract.execution_status !== "not_run") {
  failures.push("source contract must not claim an unexecuted real-Iggy result");
}

for (const marker of [
  '#![cfg(feature = "iggy")]',
  'const ADDRESS_ENV: &str = "RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS";',
  'const USERNAME_ENV: &str = "RUSTOK_IGGY_EXTERNAL_TEST_USERNAME";',
  'const PASSWORD_ENV: &str = "RUSTOK_IGGY_EXTERNAL_TEST_PASSWORD";',
  "IggyMode::External",
  'protocol: "tcp".to_string()',
  'stream_name: unique_name("stream")',
  "domain_partitions: 1",
  "replication_factor: 1",
  'open_persistent_contract_consumer_group(&source_group, "domain")',
  'open_consumer_group(&stream, "dlq", &dlq_group)',
  "PersistentContractDelivery::DecodeFailure(failure)",
  "ContractDecodeFailureKind::Deserialize",
  '"iggy.contract.decode_invalid"',
  "first_failure.ack_token().is_some()",
  "transport.move_to_dlq(first_failure.to_dlq_entry(1))",
  "drop(first_source_cursor)",
  "redelivered.offset(), first_offset",
  "redelivered.delivery_id(), first_delivery_id",
  "redelivered.raw_payload(), first_dlq.payload.as_slice()",
  ".acknowledge_decode_failure(&redelivered)",
  "second_failure.offset() > first_offset",
  "second_failure.delivery_id(), first_delivery_id",
  "transport.move_to_dlq(second_failure.to_dlq_entry(1))",
  ".acknowledge_decode_failure(&second_failure)",
  "fixture_connector.shutdown().await?",
  "transport.shutdown().await?",
  "timeout(RECEIVE_TIMEOUT",
]) {
  requireText("external Iggy raw poison test", test, marker);
}

requireOrdered("first raw poison lifecycle", test, [
  "let first_failure = receive_decode_failure(&first_source_cursor).await?;",
  "transport.move_to_dlq(first_failure.to_dlq_entry(1)).await?;",
  "let first_dlq = receive_cursor_message(&mut dlq_cursor).await?;",
  "acknowledge_cursor_message(&mut dlq_cursor, &first_dlq).await?;",
  "drop(first_source_cursor);",
  "let redelivered = receive_decode_failure(&reopened_source_cursor).await?;",
  ".acknowledge_decode_failure(&redelivered)",
  "let second_failure = receive_decode_failure(&reopened_source_cursor).await?;",
]);
requireOrdered("second raw poison lifecycle", test, [
  "let second_failure = receive_decode_failure(&reopened_source_cursor).await?;",
  "transport.move_to_dlq(second_failure.to_dlq_entry(1)).await?;",
  "let second_dlq = receive_cursor_message(&mut dlq_cursor).await?;",
  "acknowledge_cursor_message(&mut dlq_cursor, &second_dlq).await?;",
  ".acknowledge_decode_failure(&second_failure)",
]);

for (const forbidden of [
  "127.0.0.1:8090",
  'username: "iggy"',
  'password: "iggy"',
  "IggyClient",
  "IggyConsumer",
  "use iggy::",
  ".store_offset(",
  "std::thread::sleep",
  "tokio::time::sleep",
  "DELETE STREAM",
  "delete_stream",
  "DATABASE_URL",
  "ConsumerPoisonReceiptStore",
  "mark_published",
  "mark_acknowledged",
]) {
  forbidText("external Iggy raw poison test", test, forbidden);
}

const publishFixtureStart = test.indexOf("async fn publish_fixture(");
const receiveHelperStart = test.indexOf("async fn receive_decode_failure(");
const publishFixture =
  publishFixtureStart >= 0 && receiveHelperStart > publishFixtureStart
    ? test.slice(publishFixtureStart, receiveHelperStart)
    : "";
for (const marker of [
  "ExternalConnector",
  ".publish(PublishRequest::new(",
  '"domain"',
  '"raw-poison-fixture"',
]) {
  requireText("malformed fixture injector", publishFixture, marker);
}
for (const forbidden of [
  "move_to_dlq",
  "acknowledge_decode_failure",
  "ConsumerPoisonReceiptStore",
  "tenant_id",
  "event_id()",
]) {
  forbidText("malformed fixture injector", publishFixture, forbidden);
}

if (failures.length > 0) {
  console.error("External Iggy raw poison evidence verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "External Iggy raw poison source evidence verified: unique external topology, production typed receive and DLQ paths, no-ack redelivery, explicit cursor advancement, exact-byte DLQ, and bounded unexecuted claims are locked.",
);
