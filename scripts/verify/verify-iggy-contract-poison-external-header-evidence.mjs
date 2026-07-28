#!/usr/bin/env node

import { readFileSync } from "node:fs";

const contract = JSON.parse(
  readFileSync(
    "crates/rustok-iggy/contracts/evidence/contract-poison-external-iggy-header-source.json",
    "utf8",
  ),
);
const cargo = readFileSync("crates/rustok-iggy/Cargo.toml", "utf8");
const test = readFileSync(
  "crates/rustok-iggy/tests/contract_poison_external_iggy_header.rs",
  "utf8",
);
const failures = [];

const expectedSequence = [
  "start_production_transport_and_topology",
  "open_sdk_dlq_probe_group_before_publication",
  "construct_nonempty_synthetic_decode_failure_with_production_type",
  "derive_dlq_entry_and_expected_connector_uuid",
  "derive_expected_one_based_partition_from_uuid_modulo_partition_count",
  "publish_once_through_iggy_transport_move_to_dlq",
  "receive_one_physical_dlq_message_through_sdk_probe",
  "assert_physical_header_id_equals_uuid_as_u128",
  "assert_received_partition_equals_expected_one_based_partition",
  "assert_physical_payload_is_exact",
  "commit_probe_header_offset_only",
  "shutdown_production_transport",
];
const expectedProductionPaths = [
  "ConsumedContractDecodeFailure::new",
  "ConsumedContractDecodeFailure::to_dlq_entry",
  "DlqEntry::broker_message_id",
  "IggyTransport::new",
  "IggyTransport::move_to_dlq",
  "IggyTransport::shutdown",
];
const expectedProbeBoundary = {
  purpose: "observe_physical_dlq_message_only",
  allowed_operations: [
    "connect",
    "open_dlq_consumer_group",
    "receive_one_message",
    "read_header_id",
    "read_partition_id",
    "read_payload",
    "store_probe_offset",
  ],
  forbidden_operations: [
    "publish",
    "create_source_fixture",
    "mutate_production_receipt",
    "acknowledge_source_cursor",
    "delete_stream",
    "change_broker_deduplication",
  ],
};
const expectedForbiddenClaims = [
  "source_cursor_lifecycle_proved_by_this_test",
  "database_receipt_ordering_proved",
  "broker_deduplication_runtime_proved",
  "duplicate_suppression_window_proved",
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

if (contract.schema_version !== 1) failures.push("physical header contract schema drift");
if (contract.module !== "iggy") failures.push("physical header contract module drift");
if (contract.packet !== "contract-poison-external-iggy-header-source") {
  failures.push("physical header contract packet drift");
}
if (contract.status !== "source_complete_runtime_pending") {
  failures.push("physical header contract must remain runtime pending");
}
if (contract.scope !== "external_real_iggy_physical_dlq_header_and_partition") {
  failures.push("physical header contract scope drift");
}
if (contract.test_target !== "contract_poison_external_iggy_header") {
  failures.push("physical header test target drift");
}
if (
  contract.test_case !==
  "deterministic_dlq_uuid_is_physical_iggy_header_and_selects_one_based_partition"
) {
  failures.push("physical header test case drift");
}
if (
  contract.source_path !==
  "crates/rustok-iggy/tests/contract_poison_external_iggy_header.rs"
) {
  failures.push("physical header source path drift");
}
if (
  contract.verifier !==
  "scripts/verify/verify-iggy-contract-poison-external-header-evidence.mjs"
) {
  failures.push("physical header verifier path drift");
}
if (!sameValue(contract.required_environment, ["RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS"])) {
  failures.push("physical header required environment drift");
}
if (
  !sameValue(contract.optional_environment, [
    "RUSTOK_IGGY_EXTERNAL_TEST_USERNAME",
    "RUSTOK_IGGY_EXTERNAL_TEST_PASSWORD",
  ])
) {
  failures.push("physical header optional environment drift");
}
if (
  !sameValue(contract.topology, {
    mode: "external",
    protocol: "tcp",
    unique_stream_per_run: true,
    domain_partitions: 3,
    replication_factor: 1,
    observed_topic: "dlq",
    broker_requirement: "disposable_or_operator_cleaned_external_iggy",
  })
) {
  failures.push("physical header topology drift");
}
if (!sameValue(contract.required_sequence, expectedSequence)) {
  failures.push("physical header sequence drift");
}
if (!sameValue(contract.production_paths, expectedProductionPaths)) {
  failures.push("physical header production path drift");
}
if (!sameValue(contract.sdk_probe_boundary, expectedProbeBoundary)) {
  failures.push("physical header SDK probe boundary drift");
}
if (contract.publication_count !== 1) {
  failures.push("physical header evidence must contain exactly one production publication");
}
if (!sameValue(contract.forbidden_claims, expectedForbiddenClaims)) {
  failures.push("physical header forbidden claim drift");
}
if (contract.execution_status !== "not_run") {
  failures.push("physical header source contract must not claim execution");
}

requireText("rustok-iggy dev dependencies", cargo, "futures-util.workspace = true");

for (const marker of [
  '#![cfg(feature = "iggy")]',
  "use futures_util::StreamExt;",
  "use iggy::prelude::{Client, IggyClient};",
  'const ADDRESS_ENV: &str = "RUSTOK_IGGY_EXTERNAL_TEST_ADDRESS";',
  'const USERNAME_ENV: &str = "RUSTOK_IGGY_EXTERNAL_TEST_USERNAME";',
  'const PASSWORD_ENV: &str = "RUSTOK_IGGY_EXTERNAL_TEST_PASSWORD";',
  "const PARTITIONS: u32 = 3;",
  "IggyMode::External",
  'protocol: "tcp".to_string()',
  'stream_name: unique_name("header-stream")',
  "domain_partitions: PARTITIONS",
  'consumer_group(&probe_group, &stream, "dlq")',
  ".commit_failed_messages()",
  "probe.init().await?;",
  "ConsumedContractDecodeFailure::new(",
  ".with_offset(42)",
  '.with_ack_token("physical-header-evidence-only")',
  "let entry = failure.to_dlq_entry(1);",
  ".broker_message_id()",
  "expected_partition(expected_id, PARTITIONS)",
  "transport.move_to_dlq(entry).await?;",
  "timeout(RECEIVE_TIMEOUT, probe.next())",
  "received.message.header.id, expected_id.as_u128()",
  "received.partition_id, expected_partition",
  "received.message.payload.as_ref(), payload.as_slice()",
  "received.message.header.offset",
  "Some(received.partition_id)",
  "transport.shutdown().await?;",
  "(message_id.as_u128() % u128::from(partitions)) as u32 + 1",
  "(1..=partitions).contains(&partition)",
]) {
  requireText("physical Iggy header test", test, marker);
}

requireOrdered("physical header publication and observation", test, [
  "let transport = IggyTransport::new(config.clone()).await?;",
  "let client = connect_sdk_probe(&config).await?;",
  "probe.init().await?;",
  "let failure = synthetic_decode_failure(&stream, payload.clone())?;",
  "let entry = failure.to_dlq_entry(1);",
  "let expected_id = entry",
  "let expected_partition = expected_partition(expected_id, PARTITIONS)?;",
  "transport.move_to_dlq(entry).await?;",
  "let received = timeout(RECEIVE_TIMEOUT, probe.next())",
  "assert_eq!(received.message.header.id, expected_id.as_u128());",
  "assert_eq!(received.partition_id, expected_partition);",
  "assert_eq!(received.message.payload.as_ref(), payload.as_slice());",
  ".store_offset(",
  "transport.shutdown().await?;",
]);

const moveToDlqCount = [...test.matchAll(/\.move_to_dlq\(/gu)].length;
if (moveToDlqCount !== 1) {
  failures.push(
    `physical header evidence must publish exactly once through move_to_dlq; found ${moveToDlqCount}`,
  );
}
const probeNextCount = [...test.matchAll(/probe\.next\(\)/gu)].length;
if (probeNextCount !== 1) {
  failures.push(`physical header evidence must read exactly one probe message; found ${probeNextCount}`);
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
  "PersistentContractConsumerGroup",
  "open_persistent_contract_consumer_group",
  "acknowledge_decode_failure",
  "ConsumerPoisonReceiptStore",
  "mark_published",
  "mark_acknowledged",
  "dedup",
  "duplicate",
  "exactly_once",
  "delete_stream",
  "std::thread::sleep",
  "tokio::time::sleep",
  "DATABASE_URL",
  "println!(connection_string",
  "tracing::",
]) {
  forbidText("physical Iggy header test", test, forbidden);
}

const sdkProbeStart = test.indexOf("async fn connect_sdk_probe(");
const configStart = test.indexOf("fn connection_string(", sdkProbeStart);
const sdkProbe =
  sdkProbeStart >= 0 && configStart > sdkProbeStart
    ? test.slice(sdkProbeStart, configStart)
    : "";
for (const marker of [
  "IggyClient::from_connection_string",
  "client.connect().await?",
]) {
  requireText("physical Iggy SDK probe connector", sdkProbe, marker);
}
for (const forbidden of [
  ".producer(",
  ".send(",
  "IggyMessage",
  "move_to_dlq",
  "store_offset",
]) {
  forbidText("physical Iggy SDK probe connector", sdkProbe, forbidden);
}

if (failures.length > 0) {
  console.error("External Iggy physical header evidence verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "External Iggy physical header source evidence verified: one production DLQ publication, exact UUID u128 header, one-based deterministic partition, exact payload, probe-only SDK access, and bounded unexecuted claims are locked.",
);
