#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-fair-window-external-scan-runtime-source.json";
const testPath =
  "crates/rustok-iggy/tests/dlq_duplicate_fair_window_external_scan.rs";
const scannerPath = "crates/rustok-iggy/src/dlq_duplicate_external_scan.rs";
const publisherPath = "crates/rustok-iggy/src/dlq_publisher.rs";
const transportPath = "crates/rustok-iggy/src/transport.rs";
const executionContractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-fair-window-external-scan-execution-contract.json";
const runnerPath =
  "scripts/evidence/capture-iggy-dlq-duplicate-fair-window-external-scan.mjs";
const retainedVerifierPath =
  "scripts/verify/verify-iggy-dlq-duplicate-fair-window-external-scan-retained.mjs";
const evidencePath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-fair-window-external-scan-execution.json";
const expectedCase =
  "fair_window_scans_each_partition_and_differs_from_global_budget";

const contract = JSON.parse(readFileSync(resolve(root, contractPath), "utf8"));
const executionContract = JSON.parse(
  readFileSync(resolve(root, executionContractPath), "utf8"),
);
const test = readFileSync(resolve(root, testPath), "utf8");
const scanner = readFileSync(resolve(root, scannerPath), "utf8");
const publisher = readFileSync(resolve(root, publisherPath), "utf8");
const transport = readFileSync(resolve(root, transportPath), "utf8");
const runner = readFileSync(resolve(root, runnerPath), "utf8");
const retainedVerifier = readFileSync(resolve(root, retainedVerifierPath), "utf8");
const failures = [];

const same = (actual, expected) =>
  JSON.stringify(actual) === JSON.stringify(expected);
const fail = (message) => failures.push(message);
const requireText = (name, text, marker) => {
  if (!text.includes(marker)) fail(`${name} is missing: ${marker}`);
};
const forbidText = (name, text, marker) => {
  if (text.includes(marker)) fail(`${name} contains forbidden marker: ${marker}`);
};
const count = (text, marker) => text.split(marker).length - 1;

const expectedCommand = {
  program: "cargo",
  args: [
    "test",
    "-p",
    "rustok-iggy",
    "--features",
    "iggy",
    "--test",
    "dlq_duplicate_fair_window_external_scan",
    "--",
    expectedCase,
    "--exact",
    "--nocapture",
    "--test-threads=1",
  ],
};
const fairSummary = {
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
const globalSummary = {
  total_messages: 4,
  unique_message_ids: 3,
  duplicate_messages: 1,
  duplicate_groups: 1,
  conflicting_payload_groups: 0,
  max_copies_per_message_id: 2,
  has_physical_duplicates: true,
  has_identity_conflicts: false,
  requires_manual_review: false,
};

if (
  contract.schema_version !== 2 ||
  contract.module !== "iggy" ||
  contract.packet !==
    "dlq-duplicate-fair-window-external-scan-runtime-source" ||
  contract.status !== "source_complete_runtime_pending" ||
  contract.test_target !== "dlq_duplicate_fair_window_external_scan" ||
  contract.test !== testPath ||
  contract.case !== expectedCase ||
  contract.execution_status !== "not_run"
) {
  fail("fair-window runtime contract identity or status drift");
}
if (!same(contract.command, expectedCommand)) {
  fail("fair-window exact Cargo command drift");
}
if (
  contract.topology?.domain_partitions !== 2 ||
  contract.topology?.replication_factor !== 1 ||
  contract.reviewed_broker_requirement?.message_deduplication_enabled !== false ||
  contract.fixture_publication?.api !== "IggyTransport::move_to_dlq" ||
  contract.fixture_publication?.direct_sdk_producer !== false ||
  contract.fixture_publication?.physical_messages !== 5
) {
  fail("fair-window broker, topology, or fixture boundary drift");
}
if (
  contract.production_partitioning?.publisher !== "IggyDlqPublisher" ||
  contract.production_partitioning?.formula !==
    "(broker_message_id_as_u128 mod partition_count) + 1" ||
  contract.production_partitioning?.same_broker_message_id_colocated !== true ||
  contract.production_partitioning
    ?.same_broker_message_id_split_across_partitions_claimed !== false
) {
  fail("production partition invariant drift");
}
if (
  !same(contract.fair_window, {
    partitions: [1, 2],
    start_offset: 0,
    per_partition_messages: 2,
    batch_size: 2,
    runs: 2,
    same_policy_reused: true,
    required_summary: fairSummary,
  })
) {
  fail("fair-window request or summary drift");
}
if (
  !same(contract.compatibility_global_request, {
    partitions: [1, 2],
    start_offset: 0,
    max_messages: 4,
    batch_size: 2,
    runs: 1,
    required_summary: globalSummary,
    must_differ_from_fair_window: true,
  })
) {
  fail("compatibility global request or summary drift");
}
if (
  !same(contract.offset_non_mutation?.partitions, [1, 2]) ||
  !same(contract.offset_non_mutation?.checks, [
    "before_fixture_publication",
    "after_first_fair_window",
    "after_global_request",
    "after_second_fair_window",
  ]) ||
  contract.offset_non_mutation?.required_stored_offset !== null ||
  contract.offset_non_mutation?.auto_commit !== false
) {
  fail("fair-window absent-offset contract drift");
}

if (
  contract.retained_execution?.status !==
    "capture_source_complete_execution_pending" ||
  contract.retained_execution?.contract !== executionContractPath ||
  contract.retained_execution?.runner !== runnerPath ||
  contract.retained_execution?.verifier !== retainedVerifierPath ||
  contract.retained_execution?.evidence_path !== evidencePath ||
  contract.retained_execution?.canonical_packet_present !== false ||
  contract.retained_execution?.no_clobber_write !== true ||
  executionContract.packet !==
    "dlq-duplicate-fair-window-external-scan-execution-contract" ||
  executionContract.status !== "runtime_execution_contract_locked" ||
  executionContract.source_contract !== contractPath ||
  executionContract.runner !== runnerPath ||
  executionContract.verifier !== retainedVerifierPath ||
  executionContract.evidence_path !== evidencePath ||
  executionContract.evidence_status !== "runtime_execution_pending" ||
  executionContract.case !== expectedCase ||
  !same(executionContract.command, expectedCommand)
) {
  fail("fair-window retained execution relationship drift");
}

for (const marker of [
  '#![cfg(feature = "iggy")]',
  `async fn ${expectedCase}(`,
  "const PARTITIONS: [u32; 2] = [1, 2];",
  "IggyDlqDuplicateScanWindowPolicy::new(",
  "IggyDlqDuplicateScanRequest::new(",
  "scanner.summarize_window(&fair_policy).await?",
  "scanner.summarize(&global_request).await?",
  "assert_ne!(global, first_fair);",
  "assert_eq!(second_fair, first_fair);",
  "broker_message_id_for_partition(2, 1)",
  "broker_message_id_for_partition(4, 1)",
  "broker_message_id_for_partition(1, 2)",
  "Uuid::from_u128(value)",
  "candidate.as_u128() % u128::from(PARTITION_COUNT)",
  "transport.move_to_dlq(entry).await?;",
  ".get_consumer_offset(consumer, stream_id, topic_id, Some(partition))",
  "domain_partitions: PARTITION_COUNT",
]) {
  requireText("fair-window runtime harness", test, marker);
}
if (count(test, "#[tokio::test]") !== 1) {
  fail("fair-window harness must contain exactly one runtime case");
}
if (count(test, "publish_physical(") !== 6) {
  fail("fair-window harness must define and invoke five publications");
}
if (count(test, "assert_no_stored_offsets(") !== 5) {
  fail("fair-window harness must define and invoke four offset checkpoints");
}
if (count(test, "summarize_window(&fair_policy)") !== 2) {
  fail("fair-window policy must run twice");
}
if (count(test, "summarize(&global_request)") !== 1) {
  fail("compatibility global request must run once");
}
for (const marker of [
  "PublishRequest",
  "ExternalConnector",
  ".send_messages(",
  ".store_consumer_offset(",
  ".delete_consumer_offset(",
  ".consumer_group(",
  ".acknowledge(",
  ".store_offset(",
  ".delete_stream(",
  ".delete_topic(",
  ".purge_topic(",
]) {
  forbidText("fair-window runtime harness", test, marker);
}

for (const marker of [
  "pub struct IggyDlqDuplicateScanWindowPolicy",
  "pub async fn summarize_window(",
  "observations.extend(self.collect_observations(&request).await?);",
  "&PollingStrategy::offset(next_offset)",
  "requested_count,\n                        false,",
]) {
  requireText("fair-window scanner", scanner, marker);
}
for (const marker of [
  "let partition = partition_for_message_id(message_id, self.partitions);",
  ".partitioning(Partitioning::partition_id(partition))",
  ".id(message_id.as_u128())",
  "(message_id.as_u128() % u128::from(partitions)) as u32 + 1",
]) {
  requireText("deterministic DLQ publisher", publisher, marker);
}
for (const marker of [
  "if entry.broker_message_id().is_some()",
  "IggyDlqPublisher::connect",
  ".publish(&entry)",
]) {
  requireText("production transport", transport, marker);
}
for (const marker of [
  "ensureCleanCommit()",
  "sourceHashes()",
  "requirePassedCase(output)",
  "writeNoClobber({",
  'flag: "wx"',
  "linkSync(temporaryPath, outputPath)",
  "required_fair_summary: contract.required_fair_summary",
  "required_global_summary: contract.required_global_summary",
  "required_offset_observations: contract.required_offset_observations",
]) {
  requireText("fair-window retained runner", runner, marker);
}
for (const marker of [
  "currentSourceHashes()",
  "currentCommit()",
  "canonical execution JSON is absent",
  "const forbiddenKeys = new Set(contract.privacy_exclusions ?? [])",
  "fair-window execution source hashes are stale",
]) {
  requireText("fair-window retained verifier", retainedVerifier, marker);
}

const requiredNonClaims = new Set([
  "runtime_executed",
  "retained_runtime_evidence",
  "active_server_configuration_readback",
  "production_history_complete",
  "production_dedup_window_sufficiency",
  "same_broker_message_id_split_across_partitions",
  "moving_cursor",
  "cross_cycle_duplicate_accumulation",
  "bundled_iggy",
  "tls_auth_failover",
  "destructive_reconciliation",
  "profiles_authorization",
]);
for (const item of contract.non_claims ?? []) requiredNonClaims.delete(item);
if (requiredNonClaims.size) {
  fail(`fair-window non-claims are incomplete: ${[...requiredNonClaims].join(", ")}`);
}

if (
  contract.verifier !==
    "scripts/verify/verify-iggy-dlq-duplicate-fair-window-external-scan-runtime.mjs" ||
  contract.documentation !==
    "crates/rustok-iggy/docs/dlq-duplicate-fair-window-external-scan-runtime-evidence.md" ||
  contract.profiles_checkpoint !==
    "crates/rustok-profiles/docs/poison-duplicate-fair-window-external-runtime-checkpoint.md"
) {
  fail("fair-window evidence path drift");
}

if (failures.length) {
  console.error("Iggy fair-window external runtime source verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Iggy fair-window external runtime source verified: production-routed two-partition fair/global comparison, repeated fixed-window equality, absent offsets, deterministic same-ID colocation, clean-commit no-clobber retained capture, privacy-safe retained verification, and no moving-cursor or Profiles claim are locked; runtime execution remains pending.",
);
