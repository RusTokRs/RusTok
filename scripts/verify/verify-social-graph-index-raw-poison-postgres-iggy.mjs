#!/usr/bin/env node

import { readFileSync } from "node:fs";

const contractPath =
  "crates/rustok-social-graph/contracts/evidence/index-raw-poison-postgres-iggy-source.json";
const cargoPath = "crates/rustok-social-graph/Cargo.toml";
const testPath = "crates/rustok-social-graph/tests/index_raw_poison_postgres_iggy.rs";
const workerPath = "apps/server/src/services/social_graph_index_worker.rs";

const contract = JSON.parse(readFileSync(contractPath, "utf8"));
const cargo = readFileSync(cargoPath, "utf8");
const test = readFileSync(testPath, "utf8");
const worker = readFileSync(workerPath, "utf8");
const failures = [];

const expectedCases = [
  {
    case: "raw_poison_persists_published_before_source_acknowledgement",
    required_sequence: [
      "open_source_and_dlq_groups_before_fixture_publication",
      "publish_two_distinct_malformed_source_payloads",
      "receive_first_decode_failure_without_source_ack",
      "reserve_and_claim_neutral_receipt",
      "assert_receipt_publishing",
      "publish_exact_bytes_through_iggy_transport",
      "observe_and_ack_physical_dlq_payload",
      "assert_receipt_still_publishing_after_broker_publish",
      "mark_receipt_published",
      "assert_receipt_published_before_source_ack",
      "acknowledge_source_decode_failure",
      "assert_receipt_still_published_after_source_ack",
      "mark_receipt_acknowledged",
      "receive_second_source_offset",
    ],
  },
  {
    case: "published_redelivery_is_acknowledgement_only_without_republication",
    required_sequence: [
      "reserve_claim_publish_and_mark_published",
      "shutdown_first_transport_without_source_ack",
      "reopen_same_stream_and_consumer_group",
      "receive_same_offset_bytes_and_delivery_uuid",
      "reserve_and_claim_returns_already_published",
      "acknowledge_redelivered_source_without_dlq_republish",
      "mark_receipt_acknowledged",
      "observe_no_second_dlq_message",
      "receive_next_source_offset",
    ],
  },
];
const expectedProductionPaths = [
  "SocialGraphIndexConsumer::open",
  "SocialGraphIndexConsumer::receive_delivery",
  "ConsumerPoisonReceiptStore::reserve_and_claim",
  "ConsumedContractDecodeFailure::to_dlq_entry",
  "IggyTransport::move_to_dlq",
  "ConsumerPoisonReceiptStore::mark_published",
  "SocialGraphIndexConsumer::acknowledge_decode_failure",
  "ConsumerPoisonReceiptStore::mark_acknowledged",
];
const expectedWorkerOrder = [
  "reserve_and_claim",
  "transport.move_to_dlq",
  "mark_raw_poison_published",
  "acknowledge_decode_failure",
  "mark_acknowledged",
];
const expectedForbiddenClaims = [
  "physical_exactly_once_proved",
  "database_broker_transaction_proved",
  "deduplication_window_sufficiency_proved",
  "bundled_mode_proved",
  "tls_or_auth_failure_proved",
  "multi_replica_claim_ownership_proved",
  "profiles_authorization_proved",
];

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

function requireOrdered(name, source, markers) {
  let previous = -1;
  for (const marker of markers) {
    const position = source.indexOf(marker, previous + 1);
    if (position < 0) {
      fail(`${name} is missing ordered marker: ${marker}`);
      return;
    }
    if (position <= previous) {
      fail(`${name} has invalid ordering at marker: ${marker}`);
      return;
    }
    previous = position;
  }
}

function functionSlice(source, startMarker, nextMarker) {
  const start = source.indexOf(startMarker);
  if (start < 0) {
    fail(`missing function marker: ${startMarker}`);
    return "";
  }
  const end = source.indexOf(nextMarker, start + startMarker.length);
  if (end < 0) {
    fail(`missing following function marker: ${nextMarker}`);
    return source.slice(start);
  }
  return source.slice(start, end);
}

if (contract.schema_version !== 1) fail("ordering contract schema drift");
if (contract.module !== "social-graph") fail("ordering contract module drift");
if (contract.packet !== "index-raw-poison-postgres-iggy-source") {
  fail("ordering contract packet drift");
}
if (contract.status !== "source_complete_runtime_pending") {
  fail("ordering contract must remain runtime pending until execution");
}
if (contract.scope !== "social_graph_index_raw_poison_postgres_iggy_ordering") {
  fail("ordering contract scope drift");
}
if (contract.test_target !== "index_raw_poison_postgres_iggy") {
  fail("ordering test target drift");
}
if (contract.source_path !== testPath || contract.worker_path !== workerPath) {
  fail("ordering source or worker path drift");
}
if (contract.verifier !== "scripts/verify/verify-social-graph-index-raw-poison-postgres-iggy.mjs") {
  fail("ordering verifier path drift");
}
if (
  !sameValue(contract.required_environment, [
    "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_DATABASE_URL",
    "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_ADDRESS",
  ])
) {
  fail("ordering required environment drift");
}
if (
  !sameValue(contract.optional_environment, [
    "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_USERNAME",
    "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_PASSWORD",
  ])
) {
  fail("ordering optional environment drift");
}
if (
  !sameValue(contract.database, {
    backend: "postgresql",
    unique_schema_per_case: true,
    connector_migrations_only: true,
    max_connections_per_pool: 1,
    cleanup: "drop_unique_schema_only",
  })
) {
  fail("ordering database boundary drift");
}
if (
  !sameValue(contract.broker, {
    mode: "external",
    protocol: "tcp",
    unique_stream_per_case: true,
    domain_partitions: 1,
    replication_factor: 1,
    topics: ["domain", "dlq"],
    cleanup: "disposable_broker_or_operator_cleanup",
  })
) {
  fail("ordering broker boundary drift");
}
if (!sameValue(contract.cases, expectedCases)) fail("ordering case contract drift");
if (!sameValue(contract.production_paths, expectedProductionPaths)) {
  fail("ordering production path drift");
}
if (!sameValue(contract.worker_order, expectedWorkerOrder)) fail("worker order contract drift");
if (!sameValue(contract.forbidden_claims, expectedForbiddenClaims)) {
  fail("ordering forbidden claims drift");
}
if (contract.execution_status !== "not_run") {
  fail("ordering source contract must not claim runtime execution");
}

requireText(
  "social-graph test dependency",
  cargo,
  'rustok-iggy-connector = { workspace = true, features = ["migrations"] }',
);
for (const marker of [
  '#![cfg(feature = "index-consumer")]',
  'const DATABASE_ENV: &str = "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_DATABASE_URL";',
  'const ADDRESS_ENV: &str = "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_ADDRESS";',
  'const USERNAME_ENV: &str = "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_USERNAME";',
  'const PASSWORD_ENV: &str = "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_TEST_IGGY_PASSWORD";',
  "max_connections(1)",
  "min_connections(1)",
  'format!(r#"CREATE SCHEMA "{schema_name}""#)',
  'format!(r#"SET search_path TO "{schema_name}", public"#)',
  'DROP SCHEMA IF EXISTS "{}" CASCADE',
  "rustok_iggy_connector::migrations::migrations()",
  "SocialGraphIndexConsumer::open",
  "ConsumerPoisonReceiptStore::new",
  "ConsumerPoisonPublishClaim::Claimed",
  "ConsumerPoisonPublishClaim::AlreadyPublished",
  "ConsumerPoisonReceiptState::Publishing",
  "ConsumerPoisonReceiptState::Published",
  "ConsumerPoisonReceiptState::Acknowledged",
  "failure.to_dlq_entry(1)",
  "first_failure.to_dlq_entry(1)",
  "acknowledge_decode_failure",
  "assert_no_second_dlq_message",
  "next_failure.offset() > first_offset",
  "timeout(RECEIVE_TIMEOUT",
]) {
  requireText("PostgreSQL/Iggy ordering harness", test, marker);
}

const firstCase = functionSlice(
  test,
  "async fn raw_poison_persists_published_before_source_acknowledgement()",
  "#[tokio::test]\nasync fn published_redelivery_is_acknowledgement_only_without_republication()",
);
requireOrdered("published-before-ack evidence", firstCase, [
  "SocialGraphIndexConsumer::open",
  'open_consumer_group(&stream, "dlq", &dlq_group)',
  "publish_fixture(&evidence.fixture, &stream, first_payload.clone()).await?;",
  "let failure = receive_decode_failure(&consumer).await?;",
  ".reserve_and_claim(",
  "ConsumerPoisonReceiptState::Publishing",
  "transport.move_to_dlq(failure.to_dlq_entry(1)).await?;",
  "let physical_dlq = receive_cursor_message(&mut dlq_cursor).await?;",
  "acknowledge_cursor_message(&mut dlq_cursor, &physical_dlq).await?;",
  "ConsumerPoisonReceiptState::Publishing",
  "store.mark_published(&identity, publisher_id).await?;",
  "ConsumerPoisonReceiptState::Published",
  "consumer.acknowledge_decode_failure(&failure).await?;",
  "ConsumerPoisonReceiptState::Published",
  "store.mark_acknowledged(&identity).await?;",
  "ConsumerPoisonReceiptState::Acknowledged",
  "let next_failure = receive_decode_failure(&consumer).await?;",
]);

const secondCase = functionSlice(
  test,
  "async fn published_redelivery_is_acknowledgement_only_without_republication()",
  "async fn publish_fixture(",
);
requireOrdered("acknowledgement-only redelivery evidence", secondCase, [
  "SocialGraphIndexConsumer::open",
  ".reserve_and_claim(",
  ".move_to_dlq(first_failure.to_dlq_entry(1))",
  "store.mark_published(&identity, publisher_id).await?;",
  "drop(first_consumer);",
  "first_transport.shutdown().await?;",
  "let reopened_transport = Arc::new(IggyTransport::new(evidence.config.clone()).await?);",
  "let redelivered = receive_decode_failure(&reopened_consumer).await?;",
  "redelivered.offset(), first_offset",
  "redelivered.delivery_id(), first_delivery_id",
  "ConsumerPoisonPublishClaim::AlreadyPublished",
  ".acknowledge_decode_failure(&redelivered)",
  "store.mark_acknowledged(&identity).await?;",
  "assert_no_second_dlq_message(&mut dlq_cursor).await?;",
  "let next_failure = receive_decode_failure(&reopened_consumer).await?;",
]);
const secondCasePublishCount = [...secondCase.matchAll(/\.move_to_dlq\(/gu)].length;
if (secondCasePublishCount !== 1) {
  fail(`acknowledgement-only case must contain one initial DLQ publication; found ${secondCasePublishCount}`);
}
const reopenPosition = secondCase.indexOf("let reopened_transport");
if (reopenPosition >= 0 && secondCase.slice(reopenPosition).includes(".move_to_dlq(")) {
  fail("acknowledgement-only recovery must not republish after transport reopen");
}

const workerProcess = functionSlice(
  worker,
  "async fn process_decode_failure(",
  "async fn mark_raw_poison_published(",
);
requireOrdered("production raw poison worker", workerProcess, [
  ".reserve_and_claim(",
  "transport.move_to_dlq(failure.to_dlq_entry(1)).await",
  "mark_raw_poison_published(",
  "acknowledge_raw_poison_result(",
]);
const workerAcknowledge = functionSlice(
  worker,
  "async fn acknowledge_raw_poison_result(",
  "async fn acknowledge_terminal_result(",
);
requireOrdered("production raw poison acknowledgement", workerAcknowledge, [
  "consumer.acknowledge_decode_failure(failure).await",
  "poison_receipts.mark_acknowledged(identity).await",
]);
for (const marker of [
  "Ok(ConsumerPoisonPublishClaim::AlreadyPublished)",
  "Ok(ConsumerPoisonPublishClaim::AlreadyAcknowledged)",
  "break true;",
  "durable neutral receipt remains published and redelivery retries acknowledgement only",
]) {
  requireText("production acknowledgement-only recovery", worker, marker);
}

for (const forbidden of [
  "127.0.0.1:8090",
  'username: "iggy"',
  'password: "iggy"',
  'env::var("DATABASE_URL")',
  "IggyClient",
  "use iggy::",
  ".producer(",
  ".send(",
  "tokio::time::sleep",
  "std::thread::sleep",
  "DELETE FROM iggy_consumer_poison_receipts",
  "TRUNCATE",
  "delete_stream",
  "CREATE DATABASE",
  "tenant_id",
  "event_id()",
  "authorization",
  "exactly_once",
]) {
  forbidText("PostgreSQL/Iggy ordering harness", test, forbidden);
}

if (failures.length > 0) {
  console.error("Social Graph raw poison PostgreSQL/Iggy verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Social Graph raw poison PostgreSQL/Iggy source evidence verified: isolated PostgreSQL receipts, real external broker bytes, durable published-before-source-ack ordering, transport restart redelivery, acknowledgement-only recovery, worker-order parity, and bounded unexecuted claims are locked.",
);
