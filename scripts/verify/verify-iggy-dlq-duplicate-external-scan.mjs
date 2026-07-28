#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-source.json";
const runtimeContractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-runtime-source.json";
const runtimeTestPath = "crates/rustok-iggy/tests/dlq_duplicate_external_scan.rs";
const sourcePath = "crates/rustok-iggy/src/dlq_duplicate_external_scan.rs";
const classifierPath = "crates/rustok-iggy/src/dlq_duplicate_inspection.rs";
const libPath = "crates/rustok-iggy/src/lib.rs";
const expectedVerifier = "scripts/verify/verify-iggy-dlq-duplicate-external-scan.mjs";
const expectedDocumentation = "crates/rustok-iggy/docs/dlq-duplicate-external-scan.md";
const expectedProfilesCheckpoint =
  "crates/rustok-profiles/docs/poison-duplicate-external-scan-checkpoint.md";
const expectedRuntimeCase =
  "bounded_scan_classifies_duplicates_and_preserves_absent_consumer_offset";
const expectedExports = [
  "IggyDlqDuplicateScanRequest",
  "IggyDlqDuplicateScanner",
  "IggyDlqDuplicateScanError",
];
const expectedTests = [
  "bounded_request_requires_unique_positive_partitions",
  "bounded_request_rejects_unbounded_counts",
  "stable_errors_do_not_expose_broker_coordinates",
];

const contract = JSON.parse(readFileSync(resolve(repoRoot, contractPath), "utf8"));
const runtimeContract = JSON.parse(
  readFileSync(resolve(repoRoot, runtimeContractPath), "utf8"),
);
const runtimeTest = readFileSync(resolve(repoRoot, runtimeTestPath), "utf8");
const source = readFileSync(resolve(repoRoot, sourcePath), "utf8");
const classifier = readFileSync(resolve(repoRoot, classifierPath), "utf8");
const lib = readFileSync(resolve(repoRoot, libPath), "utf8");
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
  contract.packet !== "dlq-duplicate-external-scan-source" ||
  contract.status !== "source_complete_runtime_pending" ||
  contract.owner !== "rustok-iggy" ||
  contract.feature !== "iggy" ||
  contract.source !== sourcePath ||
  contract.classifier_source !== classifierPath ||
  contract.execution_status !== "source_not_run"
) {
  fail("external DLQ duplicate scan contract identity or runtime-pending status drift");
}
if (!sameValue(contract.public_exports, expectedExports)) {
  fail("external DLQ duplicate scan public export allowlist drift");
}
if (!sameValue(contract.required_source_tests, expectedTests)) {
  fail("external DLQ duplicate scan focused test allowlist drift");
}
if (
  contract.verifier !== expectedVerifier ||
  contract.documentation !== expectedDocumentation ||
  contract.profiles_checkpoint !== expectedProfilesCheckpoint
) {
  fail("external DLQ duplicate scan verifier or documentation path drift");
}
if (
  contract.runtime_harness?.status !== "source_complete_execution_pending" ||
  contract.runtime_harness?.contract !== runtimeContractPath ||
  contract.runtime_harness?.test !== runtimeTestPath ||
  contract.runtime_harness?.case !== expectedRuntimeCase ||
  contract.runtime_harness?.reviewed_dedup_disabled_broker_required !== true ||
  contract.runtime_harness?.two_identical_scans !== true ||
  contract.runtime_harness?.three_absent_offset_checks !== true ||
  runtimeContract.packet !== "dlq-duplicate-external-scan-runtime-source" ||
  runtimeContract.status !== "source_complete_runtime_pending" ||
  runtimeContract.test !== runtimeTestPath ||
  runtimeContract.case !== expectedRuntimeCase ||
  runtimeContract.execution_status !== "not_run"
) {
  fail("external DLQ duplicate scan runtime harness relationship drift");
}
if (
  contract.iggy_poll_boundary?.client !== "already_connected_IggyClient_borrow" ||
  contract.iggy_poll_boundary?.topic !== "dlq" ||
  contract.iggy_poll_boundary?.consumer_kind !== "standalone_consumer" ||
  contract.iggy_poll_boundary?.consumer_name !== "rustok-dlq-duplicate-readonly-v1" ||
  contract.iggy_poll_boundary?.partition_selection !==
    "explicit_positive_unique_partition_ids" ||
  contract.iggy_poll_boundary?.polling_strategy !== "explicit_offset" ||
  contract.iggy_poll_boundary?.auto_commit !== false ||
  contract.iggy_poll_boundary?.topic_metadata_read !== false ||
  contract.iggy_poll_boundary?.connection_lifecycle_owned_by_caller !== true
) {
  fail("external DLQ duplicate scan Iggy polling boundary drift");
}
if (
  contract.request_boundary?.maximum_partitions !== 128 ||
  contract.request_boundary?.maximum_messages !== 10000 ||
  contract.request_boundary?.maximum_batch_messages !== 1000 ||
  contract.request_boundary?.same_start_offset_for_each_partition !== true ||
  contract.request_boundary?.batch_not_greater_than_scan_limit !== true
) {
  fail("external DLQ duplicate scan bounded request contract drift");
}
for (const [operation, allowed] of Object.entries(contract.mutation_boundary ?? {})) {
  if (allowed !== false) fail(`external DLQ duplicate scan mutation became allowed: ${operation}`);
}
if (
  contract.result?.type !== "DlqDuplicateSummary" ||
  contract.result?.identifier_free !== true ||
  contract.result?.payload_free !== true ||
  contract.result?.broker_coordinate_free !== true ||
  contract.result?.raw_client_error_free !== true
) {
  fail("external DLQ duplicate scan result projection drift");
}

const expectedSourceFiles = [sourcePath, classifierPath, libPath];
if (!sameValue(contract.source_files, expectedSourceFiles)) {
  fail("external DLQ duplicate scan source file allowlist drift");
}

for (const marker of [
  'const DLQ_TOPIC: &str = "dlq";',
  'const READ_ONLY_CONSUMER: &str = "rustok-dlq-duplicate-readonly-v1";',
  "const MAX_SCAN_MESSAGES: u32 = 10_000;",
  "const MAX_BATCH_MESSAGES: u32 = 1_000;",
  "const MAX_SCAN_PARTITIONS: usize = 128;",
  "pub struct IggyDlqDuplicateScanRequest",
  "validate_partitions(&partitions)?;",
  "batch_size > max_messages",
  "pub struct IggyDlqDuplicateScanner<'a>",
  "client: &'a IggyClient",
  "kind: ConsumerKind::Consumer",
  "pub async fn summarize(",
  ".poll_messages(",
  "Some(partition_id)",
  "&PollingStrategy::offset(next_offset)",
  "requested_count,\n                        false,",
  "polled.partition_id != partition_id",
  "polled.count as usize != polled.messages.len()",
  "polled.count > requested_count",
  "offset < next_offset",
  "offset <= previous",
  "Uuid::from_u128(message.header.id)",
  "message.payload.as_ref()",
  ".checked_add(1)",
  "summarize_dlq_duplicates(observations)",
  '"iggy.dlq_duplicate.scan_invalid"',
  '"iggy.dlq_duplicate.scan_failed"',
  '"iggy.dlq_duplicate.scan_response_invalid"',
  '"iggy.dlq_duplicate.scan_offset_overflow"',
]) {
  requireText("external DLQ duplicate scan source", source, marker);
}

for (const testName of expectedTests) {
  requireText("external DLQ duplicate scan source tests", source, `fn ${testName}()`);
}
if (countText(source, "#[test]") !== expectedTests.length) {
  fail("external DLQ duplicate scan source must contain exactly three focused unit tests");
}

for (const marker of [
  "ConsumerKind::ConsumerGroup",
  "PollingStrategy::next(",
  "auto_commit: true",
  ".store_consumer_offset(",
  "ConsumerOffsetClient",
  ".consumer_group(",
  ".store_offset(",
  ".acknowledge(",
  ".delete_stream(",
  ".delete_topic(",
  ".purge_topic(",
  ".send_messages(",
  ".move_to_dlq(",
  ".retry_entry(",
  ".reserve_and_claim(",
  ".release_claim(",
  ".mark_published(",
  ".mark_acknowledged(",
  ".shutdown(",
  ".get_topic(",
  ".get_topics(",
]) {
  forbidText("external DLQ duplicate scan source", source, marker);
}

for (const marker of [
  `async fn ${expectedRuntimeCase}(`,
  "IggyTransport::new(config.clone()).await?",
  "IggyDlqDuplicateScanner::new(&client, &stream)?",
  "let first = scanner.summarize(&request).await?;",
  "let second = scanner.summarize(&request).await?;",
  "assert_eq!(second, first);",
  ".get_consumer_offset(consumer, stream_id, topic_id, Some(PARTITION_ID))",
  "if stored.is_some()",
]) {
  requireText("external DLQ duplicate scan runtime harness", runtimeTest, marker);
}
if (countText(runtimeTest, "assert_no_stored_offset(") !== 4) {
  fail("external DLQ duplicate scan runtime harness must contain three offset checks");
}
for (const marker of [
  ".store_consumer_offset(",
  ".delete_consumer_offset(",
  ".store_offset(",
  ".acknowledge(",
  ".send_messages(",
  ".delete_stream(",
  ".delete_topic(",
  ".purge_topic(",
]) {
  forbidText("external DLQ duplicate scan runtime harness", runtimeTest, marker);
}

for (const marker of [
  "pub struct DlqDuplicateObservation",
  "pub struct DlqDuplicateSummary",
  "pub fn summarize_dlq_duplicates(",
  "if broker_message_id.is_nil()",
  "payload_sha256: [u8; 32]",
]) {
  requireText("transport-neutral duplicate classifier", classifier, marker);
}

requireText(
  "rustok-iggy module list",
  lib,
  '#[cfg(feature = "iggy")]\npub mod dlq_duplicate_external_scan;',
);
for (const exportName of expectedExports) {
  requireText("rustok-iggy public exports", lib, exportName);
}

const requiredPrivacyExclusions = new Set([
  "broker_address",
  "stream",
  "topic",
  "partition",
  "offset",
  "broker_message_id",
  "payload",
  "payload_sha256",
  "credential",
]);
for (const field of contract.privacy_boundary?.result_excludes ?? []) {
  requiredPrivacyExclusions.delete(field);
}
if (requiredPrivacyExclusions.size > 0) {
  fail(
    `external DLQ duplicate scan privacy exclusions are incomplete: ${[
      ...requiredPrivacyExclusions,
    ].join(", ")}`,
  );
}

const requiredRemaining = new Set([
  "retained_external_iggy_duplicate_scan_evidence",
  "operator_alert_threshold_policy",
  "authorized_destructive_reconciliation_workflow",
  "aggregate_receipt_and_duplicate_health_correlation",
]);
for (const item of contract.remaining_work ?? []) requiredRemaining.delete(item);
if (requiredRemaining.size > 0) {
  fail(`external DLQ duplicate scan remaining work drift: ${[...requiredRemaining].join(", ")}`);
}

if (failures.length > 0) {
  console.error("Iggy external DLQ duplicate scan source verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Iggy external DLQ duplicate scan source verified: borrowed client lifecycle, standalone explicit-offset polling, auto_commit=false, bounded partitions/messages/batches, strict response progress, count-only projection, stable identifier-free errors, no offset-store or destructive API, and the source-complete dedup-disabled external runtime harness with repeated scans and absent-offset checks are locked; runtime execution remains pending.",
);
