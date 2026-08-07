#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-inspection-source.json";
const sourcePath = "crates/rustok-iggy/src/dlq_duplicate_inspection.rs";
const adapterContractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-source.json";
const adapterSourcePath = "crates/rustok-iggy/src/dlq_duplicate_external_scan.rs";
const rollingContractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-rolling-window-source.json";
const rollingSourcePath = "crates/rustok-iggy/src/dlq_duplicate_rolling_window.rs";
const movingContractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-moving-window-scan-source.json";
const movingSourcePath =
  "crates/rustok-iggy/src/dlq_duplicate_moving_window_scan.rs";
const alertContractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-policy-source.json";
const alertSourcePath = "crates/rustok-iggy/src/dlq_duplicate_alert_policy.rs";
const alertRuntimeContractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-runtime-source.json";
const alertRuntimeSourcePath = "crates/rustok-iggy/src/dlq_duplicate_alert_runtime.rs";
const observerContractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-server-observer-source.json";
const observerIggySourcePath =
  "crates/rustok-iggy/src/dlq_duplicate_alert_observer.rs";
const observerServerSourcePath =
  "apps/server/src/services/event_dlq_duplicate_alert_observer.rs";
const libPath = "crates/rustok-iggy/src/lib.rs";
const decodeFailurePath = "crates/rustok-iggy/src/contract_decode_failure.rs";
const transportPath = "crates/rustok-iggy/src/transport.rs";
const receiptInspectorPath =
  "crates/rustok-iggy-connector/src/consumer_poison_inspection.rs";
const documentationPath = "crates/rustok-iggy/docs/dlq-duplicate-inspection.md";
const profilesCheckpointPath =
  "crates/rustok-profiles/docs/poison-duplicate-dlq-operations-checkpoint.md";
const verifierPath = "scripts/verify/verify-iggy-dlq-duplicate-inspection.mjs";

const contract = JSON.parse(readFileSync(resolve(repoRoot, contractPath), "utf8"));
const adapterContract = JSON.parse(
  readFileSync(resolve(repoRoot, adapterContractPath), "utf8"),
);
const rollingContract = JSON.parse(
  readFileSync(resolve(repoRoot, rollingContractPath), "utf8"),
);
const movingContract = JSON.parse(
  readFileSync(resolve(repoRoot, movingContractPath), "utf8"),
);
const alertContract = JSON.parse(
  readFileSync(resolve(repoRoot, alertContractPath), "utf8"),
);
const alertRuntimeContract = JSON.parse(
  readFileSync(resolve(repoRoot, alertRuntimeContractPath), "utf8"),
);
const observerContract = JSON.parse(
  readFileSync(resolve(repoRoot, observerContractPath), "utf8"),
);
const source = readFileSync(resolve(repoRoot, sourcePath), "utf8");
const adapterSource = readFileSync(resolve(repoRoot, adapterSourcePath), "utf8");
const rollingSource = readFileSync(resolve(repoRoot, rollingSourcePath), "utf8");
const movingSource = readFileSync(resolve(repoRoot, movingSourcePath), "utf8");
const alertSource = readFileSync(resolve(repoRoot, alertSourcePath), "utf8");
const alertRuntimeSource = readFileSync(resolve(repoRoot, alertRuntimeSourcePath), "utf8");
const observerIggySource = readFileSync(resolve(repoRoot, observerIggySourcePath), "utf8");
const observerServerSource = readFileSync(resolve(repoRoot, observerServerSourcePath), "utf8");
const lib = readFileSync(resolve(repoRoot, libPath), "utf8");
const decodeFailure = readFileSync(resolve(repoRoot, decodeFailurePath), "utf8");
const transport = readFileSync(resolve(repoRoot, transportPath), "utf8");
const receiptInspector = readFileSync(resolve(repoRoot, receiptInspectorPath), "utf8");
const documentation = readFileSync(resolve(repoRoot, documentationPath), "utf8");
const profilesCheckpoint = readFileSync(
  resolve(repoRoot, profilesCheckpointPath),
  "utf8",
);
const failures = [];

function fail(message) {
  failures.push(message);
}
function same(actual, expected) {
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

const expectedExports = [
  "DlqDuplicateObservation",
  "DlqDuplicateSummary",
  "DlqDuplicateInspectionError",
  "summarize_dlq_duplicates",
];
const expectedSummaryFields = [
  "total_messages",
  "unique_message_ids",
  "duplicate_messages",
  "duplicate_groups",
  "conflicting_payload_groups",
  "max_copies_per_message_id",
];
const expectedTests = [
  "repeated_id_and_exact_bytes_are_counted_as_physical_duplicates",
  "one_id_with_distinct_exact_bytes_requires_manual_review",
  "empty_scan_and_empty_payload_are_valid",
  "nil_message_id_is_rejected_with_stable_code",
];

if (
  contract.schema_version !== 3 ||
  contract.module !== "iggy" ||
  contract.packet !== "dlq-duplicate-inspection-source" ||
  contract.status !== "source_complete_runtime_evidence_pending" ||
  contract.owner !== "rustok-iggy" ||
  contract.source !== sourcePath ||
  contract.verifier !== verifierPath ||
  contract.documentation !== documentationPath ||
  contract.profiles_checkpoint !== profilesCheckpointPath ||
  contract.execution_status !== "source_not_run"
) {
  fail("DLQ duplicate inspection contract identity or status drift");
}
if (!same(contract.public_exports, expectedExports)) {
  fail("DLQ duplicate inspection public export allowlist drift");
}
if (!same(contract.summary_fields, expectedSummaryFields)) {
  fail("DLQ duplicate inspection summary field allowlist drift");
}
if (!same(contract.required_source_tests, expectedTests)) {
  fail("DLQ duplicate inspection source test allowlist drift");
}

if (
  contract.identity?.input !== "non_nil_deterministic_iggy_message_header_uuid" ||
  contract.identity?.payload_comparison !== "domain_separated_sha256_in_memory_only" ||
  contract.identity?.empty_payload_valid !== true ||
  contract.identity?.nil_uuid_rejected !== true ||
  contract.semantics?.duplicate_message_formula !==
    "total_messages_minus_unique_message_ids" ||
  contract.semantics?.ordinary_duplicate !==
    "same_non_nil_message_id_and_same_exact_payload_digest" ||
  contract.semantics?.identity_conflict !==
    "same_non_nil_message_id_with_more_than_one_exact_payload_digest" ||
  contract.semantics?.manual_review_required_for_identity_conflict !== true ||
  contract.semantics?.empty_scan_returns_zero_summary !== true
) {
  fail("DLQ duplicate inspection identity or semantic boundary drift");
}
for (const [operation, allowed] of Object.entries(contract.mutation_boundary ?? {})) {
  if (allowed !== false) fail(`DLQ duplicate inspection mutation became allowed: ${operation}`);
}
if (
  contract.privacy_boundary?.observation_exposes_digest !== false ||
  contract.privacy_boundary?.summary_exposes_identifiers !== false
) {
  fail("DLQ duplicate inspection privacy flags drift");
}

if (
  contract.runtime_adapter?.status !== "source_complete_runtime_pending" ||
  contract.runtime_adapter?.contract !== adapterContractPath ||
  contract.runtime_adapter?.source !== adapterSourcePath ||
  contract.runtime_adapter?.auto_commit !== false ||
  contract.runtime_adapter?.result !== "DlqDuplicateSummary" ||
  adapterContract.packet !== "dlq-duplicate-external-scan-source" ||
  adapterContract.source !== adapterSourcePath ||
  adapterContract.classifier_source !== sourcePath
) {
  fail("fixed external duplicate scan relationship drift");
}

const rolling = contract.rolling_window ?? {};
const moving = rolling.moving_scanner ?? {};
if (
  rolling.status !== "source_complete_server_composed_runtime_pending" ||
  rolling.contract !== rollingContractPath ||
  rolling.source !== rollingSourcePath ||
  rolling.result !== "DlqDuplicateRollingWindowSnapshot" ||
  rolling.complete_cycle_retention !== true ||
  rolling.history_truncated_after_eviction !== true ||
  rolling.identifier_export !== false ||
  moving.status !== "source_complete_server_composed_runtime_pending" ||
  moving.contract !== movingContractPath ||
  moving.source !== movingSourcePath ||
  moving.independent_process_local_partition_cursors !== true ||
  moving.complete_cycle_atomicity !== true ||
  moving.progress_persisted !== false ||
  moving.restart_semantics !== "reset_to_reviewed_initial_offset" ||
  moving.cursor_values_exported !== false ||
  rollingContract.status !== "source_complete_server_composed_runtime_pending" ||
  movingContract.status !== "source_complete_server_composed_runtime_pending"
) {
  fail("rolling and moving duplicate relationship drift");
}

if (
  contract.alert_policy?.status !== "source_complete_server_observer_execution_pending" ||
  contract.alert_policy?.contract !== alertContractPath ||
  contract.alert_policy?.source !== alertSourcePath ||
  contract.alert_policy?.input !== "DlqDuplicateSummary" ||
  contract.alert_policy?.result !== "DlqDuplicateAlertEvaluation" ||
  contract.alert_policy?.identity_conflict_always_critical !== true ||
  contract.alert_policy?.production_defaults !== false ||
  contract.alert_policy?.notification_dispatch !== false ||
  contract.alert_policy?.destructive_action !== false ||
  alertContract.source !== alertSourcePath ||
  alertContract.summary_source !== sourcePath
) {
  fail("DLQ duplicate alert policy relationship drift");
}

if (
  contract.alert_runtime?.status !== "source_complete_server_observer_execution_pending" ||
  contract.alert_runtime?.contract !== alertRuntimeContractPath ||
  contract.alert_runtime?.source !== alertRuntimeSourcePath ||
  contract.alert_runtime?.input !== "DlqDuplicateSummary" ||
  contract.alert_runtime?.result !== "DlqDuplicateAlertRuntimeSnapshot" ||
  contract.alert_runtime?.latest_value !== true ||
  contract.alert_runtime?.single_writer !== true ||
  contract.alert_runtime?.unavailable_clears_evaluation !== true ||
  contract.alert_runtime?.notification_dispatch !== false ||
  contract.alert_runtime?.destructive_action !== false ||
  alertRuntimeContract.source !== alertRuntimeSourcePath ||
  alertRuntimeContract.policy_source !== alertSourcePath ||
  alertRuntimeContract.summary_source !== sourcePath
) {
  fail("DLQ duplicate alert runtime relationship drift");
}

const observer = contract.server_observer ?? {};
if (
  observer.status !== "source_complete_runtime_execution_pending" ||
  observer.contract !== observerContractPath ||
  observer.iggy_source !== observerIggySourcePath ||
  observer.server_source !== observerServerSourcePath ||
  observer.memory !== "not_applicable" ||
  observer.outbox !== "not_applicable" ||
  !same(observer.outbox_iggy_modes, ["bundled", "external"]) ||
  observer.startup_failure_non_fatal !== true ||
  !same(observer.scan_modes, ["global_budget", "fair_window", "moving_window"]) ||
  observer.default_scan_mode !== "global_budget" ||
  observer.moving_window_explicit_opt_in !== true ||
  observer.moving_configuration_fail_closed !== true ||
  observer.moving_scan_failure_preserves_process_local_state !== true ||
  observer.readiness_dependency !== false ||
  observer.profiles_authorization !== false ||
  observerContract.schema_version !== 3 ||
  observerContract.scan?.moving_window_mode?.explicit_opt_in !== true ||
  observerContract.runtime?.moving_scan_failure_retries_same_state !== true
) {
  fail("DLQ duplicate server observer relationship drift");
}

const requiredExcludedFields = new Set([
  "broker_address",
  "stream",
  "topic",
  "partition",
  "offset",
  "broker_message_id",
  "payload",
  "payload_sha256",
  "receipt_identity",
  "error_code",
  "publisher_identity",
  "timestamp",
  "credential",
]);
for (const field of contract.privacy_boundary?.summary_excludes ?? []) {
  requiredExcludedFields.delete(field);
}
if (requiredExcludedFields.size > 0) {
  fail(`DLQ duplicate privacy exclusions are incomplete: ${[
    ...requiredExcludedFields,
  ].join(", ")}`);
}

for (const marker of [
  'const DLQ_PAYLOAD_DIGEST_DOMAIN: &[u8] = b"rustok.iggy.dlq.physical_payload.v1";',
  "pub struct DlqDuplicateObservation",
  "broker_message_id: Uuid",
  "payload_sha256: [u8; 32]",
  "pub fn from_payload(",
  "if broker_message_id.is_nil()",
  "hash_part(&mut hasher, DLQ_PAYLOAD_DIGEST_DOMAIN);",
  "hash_part(&mut hasher, payload);",
  "pub struct DlqDuplicateSummary",
  "pub const fn total_messages(&self)",
  "pub const fn unique_message_ids(&self)",
  "pub const fn duplicate_messages(&self)",
  "pub const fn duplicate_groups(&self)",
  "pub const fn conflicting_payload_groups(&self)",
  "pub const fn max_copies_per_message_id(&self)",
  '"iggy.dlq_duplicate.identity_invalid"',
  '"iggy.dlq_duplicate.count_overflow"',
  "BTreeMap::<Uuid, DuplicateGroup>::new()",
  "BTreeSet<[u8; 32]>",
  "Ok(DlqDuplicateSummary {",
]) {
  requireText("DLQ duplicate inspection source", source, marker);
}
for (const testName of expectedTests) {
  requireText("DLQ duplicate inspection source tests", source, `fn ${testName}()`);
}
if (countText(source, "#[test]") !== expectedTests.length) {
  fail("DLQ duplicate inspection source must contain exactly four focused tests");
}
for (const marker of [
  "pub broker_message_id:",
  "pub payload_sha256:",
  "pub fn broker_message_id(",
  "pub fn payload_sha256(",
  "Serialize",
  "Deserialize",
  ".acknowledge(",
  ".delete(",
  ".replay(",
  ".retry_entry(",
  ".reserve_and_claim(",
  ".release_claim(",
  ".mark_published(",
  ".mark_acknowledged(",
]) {
  forbidText("DLQ duplicate inspection source", source, marker);
}

for (const marker of [
  "pub struct IggyDlqDuplicateScanner<'a>",
  ".poll_messages(",
  "summarize_dlq_duplicates(observations)",
]) {
  requireText("fixed duplicate scan adapter", adapterSource, marker);
}
for (const marker of [
  "pub struct DlqDuplicateRollingWindow",
  "pub fn push_cycle(",
  "history_truncated: evicted_cycles > 0",
]) {
  requireText("rolling duplicate state", rollingSource, marker);
}
for (const marker of [
  "pub struct IggyDlqDuplicateMovingWindowState",
  "pub async fn scan_cycle(",
  "let rolling = self.rolling.push_cycle(observations)?;",
]) {
  requireText("moving duplicate scanner", movingSource, marker);
}
for (const marker of [
  "pub struct IggyDlqDuplicateAlertMovingWindowConfig",
  "pub async fn connect_moving_window(",
  "scanner.scan_cycle(state).await?",
]) {
  requireText("Iggy alert observer", observerIggySource, marker);
}
for (const marker of [
  '"moving" | "moving_window"',
  "EventDlqDuplicateAlertScanConfig::MovingWindow",
  "connected.preserves_process_local_state_after_scan_error()",
]) {
  requireText("server alert observer", observerServerSource, marker);
}

for (const marker of [
  "ConsumedContractDecodeFailure",
  "delivery_id",
  "to_dlq_entry",
]) {
  requireText("decode-failure identity source", decodeFailure, marker);
}
requireText("physical DLQ publisher", transport, "move_to_dlq");
requireText(
  "count-only poison receipt inspector",
  receiptInspector,
  "ConsumerPoisonReceiptInspector",
);
requireText("rustok-iggy exports", lib, "DlqDuplicateSummary");
requireText("rustok-iggy exports", lib, "IggyDlqDuplicateAlertMovingWindowConfig");

for (const marker of [
  "moving-window server composition is source-complete",
  "private process-local per-partition cursors",
  "global_budget",
  "moving_window",
  "runtime execution pending",
]) {
  requireText("duplicate inspection documentation", documentation, marker);
}
for (const marker of [
  "moving-window server composition is source-complete",
  "Profiles authorization boundary",
  "private cursor",
  "runtime evidence pending",
]) {
  requireText("Profiles duplicate operations checkpoint", profilesCheckpoint, marker);
}

const requiredRemaining = new Set([
  "retained_external_iggy_duplicate_scan_evidence",
  "retain_moving_window_external_runtime_evidence",
  "review_reset_frequency_and_initial_offset_per_deployment",
  "define_persistent_cursor_owner_only_if_restart_continuity_is_required",
  "telemetry_and_health_projection",
  "alert_delivery_and_suppression_outside_policy",
  "operator_ack_delete_replay_workflow_outside_inspector",
  "correlation_with_count_only_receipt_health_without_identifier_export",
]);
for (const item of contract.remaining_work ?? []) requiredRemaining.delete(item);
if (requiredRemaining.size > 0) {
  fail(`DLQ duplicate inspection remaining work drift: ${[
    ...requiredRemaining,
  ].join(", ")}`);
}

if (failures.length > 0) {
  console.error("Iggy DLQ duplicate inspection verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Iggy physical DLQ duplicate inspection source verified: count-only identity classification, fixed scanners, bounded rolling state, independent moving cursors, explicit server moving-window composition, fail-closed reviewed configuration, latest-value alert publication, privacy boundaries, and no Profiles authorization or destructive mutation are locked; runtime evidence remains pending.",
);
