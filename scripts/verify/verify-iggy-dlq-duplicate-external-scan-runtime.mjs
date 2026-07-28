#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-runtime-source.json";
const testPath = "crates/rustok-iggy/tests/dlq_duplicate_external_scan.rs";
const scannerPath = "crates/rustok-iggy/src/dlq_duplicate_external_scan.rs";
const classifierPath = "crates/rustok-iggy/src/dlq_duplicate_inspection.rs";
const dlqPath = "crates/rustok-iggy/src/dlq.rs";
const transportPath = "crates/rustok-iggy/src/transport.rs";
const expectedVerifier =
  "scripts/verify/verify-iggy-dlq-duplicate-external-scan-runtime.mjs";
const expectedDocumentation =
  "crates/rustok-iggy/docs/dlq-duplicate-external-scan-runtime-evidence.md";
const expectedProfilesCheckpoint =
  "crates/rustok-profiles/docs/poison-duplicate-external-runtime-checkpoint.md";
const expectedCase =
  "bounded_scan_classifies_duplicates_and_preserves_absent_consumer_offset";
const expectedCommand = {
  program: "cargo",
  args: [
    "test",
    "-p",
    "rustok-iggy",
    "--features",
    "iggy",
    "--test",
    "dlq_duplicate_external_scan",
    "--",
    expectedCase,
    "--exact",
    "--nocapture",
    "--test-threads=1",
  ],
};
const expectedSummary = {
  total_messages: 4,
  unique_message_ids: 2,
  duplicate_messages: 2,
  duplicate_groups: 2,
  conflicting_payload_groups: 1,
  max_copies_per_message_id: 2,
  has_physical_duplicates: true,
  has_identity_conflicts: true,
  requires_manual_review: true,
};
const expectedSources = [testPath, scannerPath, classifierPath, dlqPath, transportPath];

const contract = JSON.parse(readFileSync(resolve(repoRoot, contractPath), "utf8"));
const test = readFileSync(resolve(repoRoot, testPath), "utf8");
const scanner = readFileSync(resolve(repoRoot, scannerPath), "utf8");
const classifier = readFileSync(resolve(repoRoot, classifierPath), "utf8");
const dlq = readFileSync(resolve(repoRoot, dlqPath), "utf8");
const transport = readFileSync(resolve(repoRoot, transportPath), "utf8");
const failures = [];

function fail(message) {
  failures.push(message);
}

function sameValue(actual, expected) {
  return JSON.stringify(actual) === JSON.stringify(expected);
}

function requireText(name, text, marker) {
  if (!text.includes(marker)) fail(`${name} is missing required marker: ${marker}`);
}

function forbidText(name, text, marker) {
  if (text.includes(marker)) fail(`${name} contains forbidden marker: ${marker}`);
}

function countText(text, marker) {
  return text.split(marker).length - 1;
}

if (
  contract.schema_version !== 1 ||
  contract.module !== "iggy" ||
  contract.packet !== "dlq-duplicate-external-scan-runtime-source" ||
  contract.status !== "source_complete_runtime_pending" ||
  contract.owner !== "rustok-iggy" ||
  contract.feature !== "iggy" ||
  contract.test_target !== "dlq_duplicate_external_scan" ||
  contract.test !== testPath ||
  contract.case !== expectedCase ||
  contract.execution_status !== "not_run"
) {
  fail("external duplicate scan runtime contract identity or pending status drift");
}
if (!sameValue(contract.command, expectedCommand)) {
  fail("external duplicate scan runtime exact Cargo command drift");
}
if (!sameValue(contract.required_summary, expectedSummary)) {
  fail("external duplicate scan runtime expected count-only summary drift");
}
if (!sameValue(contract.source_files, expectedSources)) {
  fail("external duplicate scan runtime source file allowlist drift");
}
if (
  contract.verifier !== expectedVerifier ||
  contract.documentation !== expectedDocumentation ||
  contract.profiles_checkpoint !== expectedProfilesCheckpoint
) {
  fail("external duplicate scan runtime verifier or documentation path drift");
}

if (
  contract.environment?.address !== "RUSTOK_IGGY_DUPLICATE_SCAN_TEST_ADDRESS" ||
  contract.environment?.optional_username !==
    "RUSTOK_IGGY_DUPLICATE_SCAN_TEST_USERNAME" ||
  contract.environment?.optional_password !==
    "RUSTOK_IGGY_DUPLICATE_SCAN_TEST_PASSWORD" ||
  contract.environment?.default_address !== false ||
  contract.environment?.default_credentials !== false
) {
  fail("external duplicate scan runtime environment boundary drift");
}
if (
  contract.reviewed_broker_requirement?.mode !==
    "external_disposable_or_operator_cleaned" ||
  contract.reviewed_broker_requirement?.message_deduplication_enabled !== false ||
  contract.reviewed_broker_requirement?.configuration_readback_by_test !== false
) {
  fail("external duplicate scan runtime reviewed broker requirement drift");
}
if (
  contract.topology?.unique_stream_per_run !== true ||
  contract.topology?.topic !== "dlq" ||
  contract.topology?.domain_partitions !== 1 ||
  contract.topology?.replication_factor !== 1 ||
  contract.topology?.stream_deletion_by_test !== false
) {
  fail("external duplicate scan runtime topology boundary drift");
}
if (
  contract.fixture_publication?.api !== "IggyTransport::move_to_dlq" ||
  contract.fixture_publication?.direct_sdk_producer !== false ||
  contract.fixture_publication?.physical_messages !== 4 ||
  contract.fixture_publication?.ordinary_duplicate?.copies !== 2 ||
  contract.fixture_publication?.ordinary_duplicate?.same_header_uuid !== true ||
  contract.fixture_publication?.ordinary_duplicate?.same_exact_bytes !== true ||
  contract.fixture_publication?.identity_conflict?.copies !== 2 ||
  contract.fixture_publication?.identity_conflict?.same_header_uuid !== true ||
  contract.fixture_publication?.identity_conflict?.different_exact_bytes !== true
) {
  fail("external duplicate scan runtime fixture semantics drift");
}
if (
  !sameValue(contract.scan_request, {
    partitions: [1],
    start_offset: 0,
    max_messages: 4,
    batch_size: 4,
    runs: 2,
    same_request_reused: true,
  })
) {
  fail("external duplicate scan runtime request drift");
}
if (
  contract.offset_non_mutation?.consumer_kind !== "standalone_consumer" ||
  contract.offset_non_mutation?.consumer_name !==
    "rustok-dlq-duplicate-readonly-v1" ||
  contract.offset_non_mutation?.partition !== 1 ||
  !sameValue(contract.offset_non_mutation?.checks, [
    "before_fixture_publication",
    "after_first_scan",
    "after_second_scan",
  ]) ||
  contract.offset_non_mutation?.required_stored_offset !== null ||
  contract.offset_non_mutation?.second_scan_summary_equals_first !== true ||
  contract.offset_non_mutation?.auto_commit !== false
) {
  fail("external duplicate scan runtime offset non-mutation contract drift");
}
if (
  contract.privacy_boundary?.source_assertions_may_compare_exact_bytes !== false ||
  contract.privacy_boundary?.summary_only !== true
) {
  fail("external duplicate scan runtime privacy boundary drift");
}

for (const marker of [
  '#![cfg(feature = "iggy")]',
  'const ADDRESS_ENV: &str = "RUSTOK_IGGY_DUPLICATE_SCAN_TEST_ADDRESS";',
  'const USERNAME_ENV: &str = "RUSTOK_IGGY_DUPLICATE_SCAN_TEST_USERNAME";',
  'const PASSWORD_ENV: &str = "RUSTOK_IGGY_DUPLICATE_SCAN_TEST_PASSWORD";',
  'const READ_ONLY_CONSUMER: &str = "rustok-dlq-duplicate-readonly-v1";',
  "const PARTITION_ID: u32 = 1;",
  "const PHYSICAL_MESSAGE_COUNT: u32 = 4;",
  `async fn ${expectedCase}(`,
  "IggyTransport::new(config.clone()).await?",
  "IggyDlqDuplicateScanner::new(&client, &stream)?",
  "IggyDlqDuplicateScanRequest::new(",
  "vec![PARTITION_ID]",
  "ConsumerKind::Consumer",
  "let ordinary_duplicate_id = Uuid::new_v4();",
  "let identity_conflict_id = Uuid::new_v4();",
  "ordinary_payload.clone()",
  "ordinary_payload,",
  "conflict_payload_first,",
  "conflict_payload_second,",
  "let first = scanner.summarize(&request).await?;",
  "let second = scanner.summarize(&request).await?;",
  "assert_eq!(second, first);",
  "assert_eq!(summary.total_messages(), 4);",
  "assert_eq!(summary.unique_message_ids(), 2);",
  "assert_eq!(summary.duplicate_messages(), 2);",
  "assert_eq!(summary.duplicate_groups(), 2);",
  "assert_eq!(summary.conflicting_payload_groups(), 1);",
  "assert_eq!(summary.max_copies_per_message_id(), 2);",
  "assert!(summary.has_physical_duplicates());",
  "assert!(summary.has_identity_conflicts());",
  "assert!(summary.requires_manual_review());",
  ".with_broker_message_id(broker_message_id)",
  "transport.move_to_dlq(entry).await?;",
  ".get_consumer_offset(consumer, stream_id, topic_id, Some(PARTITION_ID))",
  "if stored.is_some()",
  "client.shutdown().await?;",
  "transport.shutdown().await?;",
  "domain_partitions: 1",
  "replication_factor: 1",
]) {
  requireText("external duplicate scan runtime harness", test, marker);
}
if (countText(test, "publish_physical(") !== 5) {
  fail("external duplicate scan runtime harness must define and invoke four fixture publishes");
}
if (countText(test, "assert_no_stored_offset(") !== 4) {
  fail("external duplicate scan runtime harness must define and invoke three offset checks");
}
if (countText(test, "scanner.summarize(&request).await?") !== 2) {
  fail("external duplicate scan runtime harness must repeat the same explicit-offset scan twice");
}
if (countText(test, "#[tokio::test]") !== 1) {
  fail("external duplicate scan runtime harness must contain exactly one focused runtime case");
}

for (const marker of [
  "DATABASE_URL",
  "localhost",
  "127.0.0.1",
  "PublishRequest",
  "ExternalConnector",
  ".poll_messages(",
  ".send_messages(",
  ".store_consumer_offset(",
  ".delete_consumer_offset(",
  ".consumer_group(",
  ".acknowledge(",
  ".store_offset(",
  ".delete_stream(",
  ".delete_topic(",
  ".purge_topic(",
  ".get_topic(",
  ".get_topics(",
]) {
  forbidText("external duplicate scan runtime harness", test, marker);
}

for (const marker of [
  "pub struct IggyDlqDuplicateScanner<'a>",
  "ConsumerKind::Consumer",
  "&PollingStrategy::offset(next_offset)",
  "requested_count,\n                        false,",
  "summarize_dlq_duplicates(observations)",
]) {
  requireText("bounded external duplicate scanner", scanner, marker);
}
for (const marker of [
  ".store_consumer_offset(",
  ".store_offset(",
  ".acknowledge(",
  ".send_messages(",
  ".delete_topic(",
  ".purge_topic(",
]) {
  forbidText("bounded external duplicate scanner", scanner, marker);
}

for (const marker of [
  "pub struct DlqDuplicateObservation",
  "pub struct DlqDuplicateSummary",
  "pub fn summarize_dlq_duplicates(",
  "if broker_message_id.is_nil()",
  "group.payload_sha256.len() > 1",
]) {
  requireText("transport-neutral duplicate classifier", classifier, marker);
}
for (const marker of [
  "let publish_event_id = entry.broker_message_id.unwrap_or(entry.event_id);",
  "publish_event_id.to_string()",
]) {
  requireText("DLQ deterministic physical publisher", dlq, marker);
}
for (const marker of [
  "if entry.broker_message_id().is_some()",
  "IggyDlqPublisher::connect",
  ".publish(&entry)",
]) {
  requireText("production Iggy transport", transport, marker);
}

const requiredNonClaims = new Set([
  "active_server_configuration_readback",
  "production_history_complete",
  "production_dedup_window_sufficiency",
  "stored_offset_non_mutation_runtime_executed",
  "retained_runtime_evidence",
  "bundled_iggy",
  "tls_auth_failover",
  "multi_partition_runtime",
  "destructive_reconciliation",
  "profiles_authorization",
]);
for (const claim of contract.non_claims ?? []) requiredNonClaims.delete(claim);
if (requiredNonClaims.size > 0) {
  fail(`external duplicate scan runtime non-claims are incomplete: ${[
    ...requiredNonClaims,
  ].join(", ")}`);
}

if (failures.length > 0) {
  console.error("Iggy external DLQ duplicate scan runtime source verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Iggy external DLQ duplicate scan runtime source verified: production publication of ordinary duplicate and conflicting-byte fixtures, two identical bounded explicit-offset scans, exact count-only classification, three absent-offset observations, auto_commit=false scanner parity, no SDK producer or offset mutation, reviewed dedup-disabled broker boundary, and bounded non-claims are locked; runtime execution remains pending.",
);
