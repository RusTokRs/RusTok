#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const paths = {
  contract: "crates/rustok-iggy/contracts/evidence/dlq-duplicate-inspection-source.json",
  source: "crates/rustok-iggy/src/dlq_duplicate_inspection.rs",
  adapterContract:
    "crates/rustok-iggy/contracts/evidence/dlq-duplicate-external-scan-source.json",
  adapterSource: "crates/rustok-iggy/src/dlq_duplicate_external_scan.rs",
  rollingContract:
    "crates/rustok-iggy/contracts/evidence/dlq-duplicate-rolling-window-source.json",
  rollingSource: "crates/rustok-iggy/src/dlq_duplicate_rolling_window.rs",
  movingContract:
    "crates/rustok-iggy/contracts/evidence/dlq-duplicate-moving-window-scan-source.json",
  movingSource: "crates/rustok-iggy/src/dlq_duplicate_moving_window_scan.rs",
  alertContract:
    "crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-policy-source.json",
  alertSource: "crates/rustok-iggy/src/dlq_duplicate_alert_policy.rs",
  alertRuntimeContract:
    "crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-runtime-source.json",
  alertRuntimeSource: "crates/rustok-iggy/src/dlq_duplicate_alert_runtime.rs",
  observerContract:
    "crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-server-observer-source.json",
  observerIggySource: "crates/rustok-iggy/src/dlq_duplicate_alert_observer.rs",
  observerServerSource:
    "apps/server/src/services/event_dlq_duplicate_alert_observer.rs",
  lib: "crates/rustok-iggy/src/lib.rs",
  decodeFailure: "crates/rustok-iggy/src/contract_decode_failure.rs",
  transport: "crates/rustok-iggy/src/transport.rs",
  receiptInspector:
    "crates/rustok-iggy-connector/src/consumer_poison_inspection.rs",
  documentation: "crates/rustok-iggy/docs/dlq-duplicate-inspection.md",
  profilesCheckpoint:
    "crates/rustok-profiles/docs/poison-duplicate-dlq-operations-checkpoint.md",
  verifier: "scripts/verify/verify-iggy-dlq-duplicate-inspection.mjs",
};

function read(path) {
  return readFileSync(resolve(repoRoot, path), "utf8");
}
function readJson(path) {
  return JSON.parse(read(path));
}

const contract = readJson(paths.contract);
const adapterContract = readJson(paths.adapterContract);
const rollingContract = readJson(paths.rollingContract);
const movingContract = readJson(paths.movingContract);
const alertContract = readJson(paths.alertContract);
const alertRuntimeContract = readJson(paths.alertRuntimeContract);
const observerContract = readJson(paths.observerContract);
const source = read(paths.source);
const adapterSource = read(paths.adapterSource);
const rollingSource = read(paths.rollingSource);
const movingSource = read(paths.movingSource);
const alertSource = read(paths.alertSource);
const alertRuntimeSource = read(paths.alertRuntimeSource);
const observerIggySource = read(paths.observerIggySource);
const observerServerSource = read(paths.observerServerSource);
const lib = read(paths.lib);
const decodeFailure = read(paths.decodeFailure);
const transport = read(paths.transport);
const receiptInspector = read(paths.receiptInspector);
const documentation = read(paths.documentation);
const profilesCheckpoint = read(paths.profilesCheckpoint);
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
  contract.schema_version !== 2 ||
  contract.module !== "iggy" ||
  contract.packet !== "dlq-duplicate-inspection-source" ||
  contract.status !== "source_complete_runtime_evidence_pending" ||
  contract.owner !== "rustok-iggy" ||
  contract.source !== paths.source ||
  contract.verifier !== paths.verifier ||
  contract.documentation !== paths.documentation ||
  contract.profiles_checkpoint !== paths.profilesCheckpoint ||
  contract.execution_status !== "source_not_run"
) {
  fail("DLQ duplicate inspection contract identity or status drift");
}
if (!same(contract.public_exports, expectedExports)) {
  fail("DLQ duplicate inspection export allowlist drift");
}
if (!same(contract.summary_fields, expectedSummaryFields)) {
  fail("DLQ duplicate inspection summary allowlist drift");
}
if (!same(contract.required_source_tests, expectedTests)) {
  fail("DLQ duplicate inspection test allowlist drift");
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
  contract.semantics?.manual_review_required_for_identity_conflict !== true
) {
  fail("DLQ duplicate inspection identity or semantic boundary drift");
}
for (const [operation, allowed] of Object.entries(contract.mutation_boundary ?? {})) {
  if (allowed !== false) fail(`inspection mutation became allowed: ${operation}`);
}
if (
  contract.privacy_boundary?.observation_exposes_digest !== false ||
  contract.privacy_boundary?.summary_exposes_identifiers !== false
) {
  fail("inspection privacy flags drift");
}

if (
  contract.runtime_adapter?.status !== "source_complete_runtime_pending" ||
  contract.runtime_adapter?.contract !== paths.adapterContract ||
  contract.runtime_adapter?.source !== paths.adapterSource ||
  contract.runtime_adapter?.auto_commit !== false ||
  contract.runtime_adapter?.result !== "DlqDuplicateSummary" ||
  adapterContract.packet !== "dlq-duplicate-external-scan-source" ||
  adapterContract.source !== paths.adapterSource ||
  adapterContract.classifier_source !== paths.source
) {
  fail("fixed external scanner relationship drift");
}

const rolling = contract.rolling_window ?? {};
const moving = rolling.moving_scanner ?? {};
if (
  rolling.status !== "source_complete_moving_scan_integrated_server_pending" ||
  rolling.contract !== paths.rollingContract ||
  rolling.source !== paths.rollingSource ||
  rolling.result !== "DlqDuplicateRollingWindowSnapshot" ||
  rolling.complete_cycle_retention !== true ||
  rolling.history_truncated_after_eviction !== true ||
  rolling.identifier_export !== false ||
  moving.status !== "source_complete_server_composition_runtime_pending" ||
  moving.contract !== paths.movingContract ||
  moving.source !== paths.movingSource ||
  moving.independent_process_local_partition_cursors !== true ||
  moving.complete_cycle_atomicity !== true ||
  moving.progress_persisted !== false ||
  moving.restart_semantics !== "reset_to_reviewed_initial_offset" ||
  moving.cursor_values_exported !== false ||
  rollingContract.packet !== "dlq-duplicate-rolling-window-source" ||
  rollingContract.status !== "source_complete_moving_scan_integrated_server_pending" ||
  movingContract.packet !== "dlq-duplicate-moving-window-scan-source" ||
  movingContract.status !== "source_complete_server_composition_runtime_pending"
) {
  fail("rolling or moving scanner relationship drift");
}

if (
  contract.alert_policy?.contract !== paths.alertContract ||
  contract.alert_policy?.source !== paths.alertSource ||
  contract.alert_policy?.input !== "DlqDuplicateSummary" ||
  contract.alert_policy?.identity_conflict_always_critical !== true ||
  contract.alert_policy?.production_defaults !== false ||
  alertContract.source !== paths.alertSource ||
  alertContract.summary_source !== paths.source
) {
  fail("duplicate alert policy relationship drift");
}
if (
  contract.alert_runtime?.contract !== paths.alertRuntimeContract ||
  contract.alert_runtime?.source !== paths.alertRuntimeSource ||
  contract.alert_runtime?.latest_value !== true ||
  contract.alert_runtime?.single_writer !== true ||
  contract.alert_runtime?.unavailable_clears_evaluation !== true ||
  alertRuntimeContract.source !== paths.alertRuntimeSource ||
  alertRuntimeContract.policy_source !== paths.alertSource
) {
  fail("duplicate alert runtime relationship drift");
}
if (
  contract.server_observer?.contract !== paths.observerContract ||
  contract.server_observer?.iggy_source !== paths.observerIggySource ||
  contract.server_observer?.server_source !== paths.observerServerSource ||
  contract.server_observer?.memory !== "not_applicable" ||
  contract.server_observer?.outbox_local !== "not_applicable" ||
  !same(contract.server_observer?.outbox_iggy_modes, ["bundled", "external"]) ||
  contract.server_observer?.startup_failure_non_fatal !== true ||
  contract.server_observer?.moving_window_mode !== "pending_explicit_opt_in" ||
  contract.server_observer?.readiness_dependency !== false ||
  contract.server_observer?.profiles_authorization !== false ||
  observerContract.packet !== "dlq-duplicate-alert-server-observer-source" ||
  observerContract.scan?.moving_cursor !== false ||
  observerContract.scan?.current_tail_coverage_claimed !== false ||
  observerContract.scan?.complete_history_claimed !== false
) {
  fail("server observer relationship or moving-mode pending boundary drift");
}

const requiredExcluded = new Set([
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
  requiredExcluded.delete(field);
}
if (requiredExcluded.size > 0) {
  fail(`inspection privacy exclusions incomplete: ${[...requiredExcluded].join(", ")}`);
}

for (const marker of [
  'const DLQ_PAYLOAD_DIGEST_DOMAIN: &[u8] = b"rustok.iggy.dlq.physical_payload.v1";',
  "pub struct DlqDuplicateObservation",
  "broker_message_id: Uuid",
  "payload_sha256: [u8; 32]",
  "pub fn from_payload(",
  "if broker_message_id.is_nil()",
  "pub struct DlqDuplicateSummary",
  "pub const fn requires_manual_review(&self)",
  "BTreeMap::<Uuid, DuplicateGroup>::new()",
  "BTreeSet<[u8; 32]>",
  "group.payload_sha256.len() > 1",
  '"iggy.dlq_duplicate.identity_invalid"',
  '"iggy.dlq_duplicate.count_overflow"',
]) {
  requireText("inspection source", source, marker);
}
for (const testName of expectedTests) {
  requireText("inspection tests", source, `fn ${testName}()`);
}
if (countText(source, "#[test]") !== expectedTests.length) {
  fail("inspection source must contain exactly four focused tests");
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
  ".reserve_and_claim(",
  ".mark_published(",
]) {
  forbidText("inspection source", source, marker);
}

for (const marker of [
  "pub struct IggyDlqDuplicateScanner<'a>",
  ".poll_messages(",
  "&PollingStrategy::offset(next_offset)",
  "summarize_dlq_duplicates(observations)",
]) {
  requireText("fixed scanner", adapterSource, marker);
}
for (const marker of [
  "pub struct DlqDuplicateRollingWindow",
  "pub fn push_cycle(",
  "history_truncated: evicted_cycles > 0",
]) {
  requireText("rolling source", rollingSource, marker);
}
for (const marker of [
  "pub struct IggyDlqDuplicateMovingWindowState",
  "cursors: BTreeMap<u32, u64>",
  "fn apply_complete_cycle(",
  "let rolling = self.rolling.push_cycle(observations)?;",
  "self.cursors = candidate_cursors;",
  "pub fn reset_to_initial_offset(",
]) {
  requireText("moving scanner", movingSource, marker);
}
for (const marker of [
  "pub struct DlqDuplicateAlertPolicy",
  "pub enum DlqDuplicateAlertLevel",
]) {
  requireText("alert policy", alertSource, marker);
}
for (const marker of [
  "pub struct DlqDuplicateAlertRuntimePublisher",
  "pub fn mark_unavailable(",
  "evaluation: None",
]) {
  requireText("alert runtime", alertRuntimeSource, marker);
}
requireText(
  "Iggy observer",
  observerIggySource,
  "pub struct IggyDlqDuplicateAlertObserver",
);
for (const marker of [
  "Unavailable",
  "NotApplicableMemory",
  "NotApplicableOutboxLocal",
  "IggyBundled",
  "IggyExternal",
  "publisher.mark_unavailable()",
]) {
  requireText("server observer", observerServerSource, marker);
}

for (const marker of [
  "pub mod dlq_duplicate_inspection;",
  "pub mod dlq_duplicate_rolling_window;",
  '#[cfg(feature = "iggy")]\npub mod dlq_duplicate_moving_window_scan;',
]) {
  requireText("rustok-iggy module list", lib, marker);
}
for (const exportName of expectedExports) {
  requireText("rustok-iggy exports", lib, exportName);
}

for (const marker of [
  "Failure kind, retry count, time, process identity, and random values are excluded",
  "pub fn delivery_id(&self) -> Uuid",
  "hash_part(&mut hasher, &self.raw_payload);",
  ".with_broker_message_id(delivery_id)",
]) {
  requireText("raw poison identity", decodeFailure, marker);
}
for (const marker of [
  "if entry.broker_message_id().is_some()",
  "IggyDlqPublisher::connect",
  ".publish(&entry)",
]) {
  requireText("deterministic publisher", transport, marker);
}
for (const marker of [
  "Bounded aggregate view of neutral poison-result progress",
  "The snapshot intentionally excludes delivery identifiers",
  "pub struct ConsumerPoisonReceiptSummary",
  "pub async fn summarize(",
]) {
  requireText("receipt inspector", receiptInspector, marker);
}

for (const marker of [
  "moving scanner integration is source-complete",
  "independent process-local per-partition cursors",
  "explicit restart reset",
  "server composition and runtime evidence pending",
]) {
  requireText("inspection documentation", documentation, marker);
}
for (const marker of [
  "moving scanner integration is source-complete",
  "private process-local per-partition cursors",
  "restart reset",
  "Profiles authorization boundary",
]) {
  requireText("Profiles operations checkpoint", profilesCheckpoint, marker);
}

if (
  contract.production_relationship?.raw_poison_delivery_id !==
    "ConsumedContractDecodeFailure::delivery_id" ||
  contract.production_relationship?.dlq_broker_message_id !==
    "ConsumedContractDecodeFailure::to_dlq_entry" ||
  contract.production_relationship?.physical_publisher !== "IggyTransport::move_to_dlq" ||
  contract.production_relationship?.receipt_summary !== "ConsumerPoisonReceiptInspector" ||
  contract.production_relationship?.receipt_and_physical_duplicate_summaries_are_independent !==
    true
) {
  fail("production identity relationship drift");
}

const requiredRemaining = new Set([
  "retained_external_iggy_duplicate_scan_evidence",
  "compose_moving_window_server_observer",
  "define_reviewed_moving_window_configuration",
  "retain_moving_window_external_runtime_evidence",
  "define_persistent_cursor_owner_only_if_restart_continuity_is_required",
  "telemetry_and_health_projection",
  "alert_delivery_and_suppression_outside_policy",
  "operator_ack_delete_replay_workflow_outside_inspector",
  "correlation_with_count_only_receipt_health_without_identifier_export",
]);
for (const item of contract.remaining_work ?? []) requiredRemaining.delete(item);
if (requiredRemaining.size > 0) {
  fail(`inspection remaining work drift: ${[...requiredRemaining].join(", ")}`);
}

if (failures.length > 0) {
  console.error("Iggy DLQ duplicate inspection verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Iggy DLQ duplicate inspection source verified: deterministic header identity, in-memory exact-byte digest comparison, count-only privacy, fixed and moving bounded scanners, complete-cycle atomic rolling integration, explicit non-persistent restart reset, alert/runtime/server boundaries, and Profiles non-authorization are locked; server moving-mode composition, runtime evidence, telemetry, and authorized operations remain pending.",
);
