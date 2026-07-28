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
const expectedVerifier = "scripts/verify/verify-iggy-dlq-duplicate-inspection.mjs";
const expectedDocumentation = "crates/rustok-iggy/docs/dlq-duplicate-inspection.md";
const expectedProfilesCheckpoint =
  "crates/rustok-profiles/docs/poison-duplicate-dlq-operations-checkpoint.md";
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

const contract = JSON.parse(readFileSync(resolve(repoRoot, contractPath), "utf8"));
const adapterContract = JSON.parse(
  readFileSync(resolve(repoRoot, adapterContractPath), "utf8"),
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
const alertSource = readFileSync(resolve(repoRoot, alertSourcePath), "utf8");
const alertRuntimeSource = readFileSync(resolve(repoRoot, alertRuntimeSourcePath), "utf8");
const observerIggySource = readFileSync(resolve(repoRoot, observerIggySourcePath), "utf8");
const observerServerSource = readFileSync(resolve(repoRoot, observerServerSourcePath), "utf8");
const lib = readFileSync(resolve(repoRoot, libPath), "utf8");
const decodeFailure = readFileSync(resolve(repoRoot, decodeFailurePath), "utf8");
const transport = readFileSync(resolve(repoRoot, transportPath), "utf8");
const receiptInspector = readFileSync(resolve(repoRoot, receiptInspectorPath), "utf8");
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
  contract.packet !== "dlq-duplicate-inspection-source" ||
  contract.status !== "source_complete_runtime_evidence_pending" ||
  contract.owner !== "rustok-iggy" ||
  contract.source !== sourcePath ||
  contract.execution_status !== "source_not_run"
) {
  fail("DLQ duplicate inspection contract identity or source status drift");
}
if (!sameValue(contract.public_exports, expectedExports)) {
  fail("DLQ duplicate inspection public export allowlist drift");
}
if (!sameValue(contract.summary_fields, expectedSummaryFields)) {
  fail("DLQ duplicate inspection summary field allowlist drift");
}
if (!sameValue(contract.required_source_tests, expectedTests)) {
  fail("DLQ duplicate inspection source test allowlist drift");
}
if (
  contract.verifier !== expectedVerifier ||
  contract.documentation !== expectedDocumentation ||
  contract.profiles_checkpoint !== expectedProfilesCheckpoint
) {
  fail("DLQ duplicate inspection verifier or documentation path drift");
}

if (
  contract.identity?.input !== "non_nil_deterministic_iggy_message_header_uuid" ||
  contract.identity?.payload_comparison !== "domain_separated_sha256_in_memory_only" ||
  contract.identity?.empty_payload_valid !== true ||
  contract.identity?.nil_uuid_rejected !== true
) {
  fail("DLQ duplicate inspection identity boundary drift");
}
if (
  contract.semantics?.duplicate_message_formula !==
    "total_messages_minus_unique_message_ids" ||
  contract.semantics?.ordinary_duplicate !==
    "same_non_nil_message_id_and_same_exact_payload_digest" ||
  contract.semantics?.identity_conflict !==
    "same_non_nil_message_id_with_more_than_one_exact_payload_digest" ||
  contract.semantics?.manual_review_required_for_identity_conflict !== true ||
  contract.semantics?.empty_scan_returns_zero_summary !== true
) {
  fail("DLQ duplicate inspection semantic contract drift");
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
  fail("DLQ duplicate inspection runtime adapter relationship drift");
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
  alertContract.status !== "source_complete_server_observer_execution_pending" ||
  alertContract.source !== alertSourcePath ||
  alertContract.summary_source !== sourcePath
) {
  fail("DLQ duplicate inspection alert policy relationship drift");
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
  alertRuntimeContract.status !== "source_complete_server_observer_execution_pending" ||
  alertRuntimeContract.source !== alertRuntimeSourcePath ||
  alertRuntimeContract.policy_source !== alertSourcePath ||
  alertRuntimeContract.summary_source !== sourcePath
) {
  fail("DLQ duplicate inspection alert runtime relationship drift");
}
if (
  contract.server_observer?.status !== "source_complete_runtime_execution_pending" ||
  contract.server_observer?.contract !== observerContractPath ||
  contract.server_observer?.iggy_source !== observerIggySourcePath ||
  contract.server_observer?.server_source !== observerServerSourcePath ||
  contract.server_observer?.memory !== "not_applicable" ||
  contract.server_observer?.outbox_local !== "not_applicable" ||
  !sameValue(contract.server_observer?.outbox_iggy_modes, ["bundled", "external"]) ||
  contract.server_observer?.readiness_dependency !== false ||
  contract.server_observer?.profiles_authorization !== false ||
  observerContract.packet !== "dlq-duplicate-alert-server-observer-source" ||
  observerContract.status !== "source_complete_runtime_execution_pending" ||
  observerContract.iggy_source !== observerIggySourcePath ||
  observerContract.server_source !== observerServerSourcePath
) {
  fail("DLQ duplicate inspection server observer relationship drift");
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
  "pub const fn has_physical_duplicates(&self)",
  "pub const fn has_identity_conflicts(&self)",
  "pub const fn requires_manual_review(&self)",
  '"iggy.dlq_duplicate.identity_invalid"',
  '"iggy.dlq_duplicate.count_overflow"',
  "BTreeMap::<Uuid, DuplicateGroup>::new()",
  "BTreeSet<[u8; 32]>",
  ".checked_add(1)",
  ".checked_sub(unique_message_ids)",
  "group.payload_sha256.len() > 1",
  "Ok(DlqDuplicateSummary {",
]) {
  requireText("DLQ duplicate inspection source", source, marker);
}
for (const testName of expectedTests) {
  requireText("DLQ duplicate inspection source tests", source, `fn ${testName}()`);
}
if (countText(source, "#[test]") !== expectedTests.length) {
  fail("DLQ duplicate inspection source must contain exactly four focused unit tests");
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
  "&PollingStrategy::offset(next_offset)",
  "requested_count,\n                        false,",
  "summarize_dlq_duplicates(observations)",
]) {
  requireText("bounded external duplicate scan adapter", adapterSource, marker);
}
for (const marker of [
  "pub struct DlqDuplicateAlertPolicy",
  "pub const fn evaluate(",
  "pub enum DlqDuplicateAlertLevel",
  "pub struct DlqDuplicateAlertEvaluation",
]) {
  requireText("count-only duplicate alert policy", alertSource, marker);
}
for (const marker of [
  "pub struct DlqDuplicateAlertRuntimePublisher",
  "pub struct DlqDuplicateAlertRuntimeSnapshot",
  "pub fn mark_unavailable(",
  "evaluation: None",
]) {
  requireText("latest-value alert runtime", alertRuntimeSource, marker);
}
for (const marker of [
  "pub struct IggyDlqDuplicateAlertObserver",
  "IggyDlqDuplicateScanner::new(&self.client, &self.stream_name)?",
]) {
  requireText("Iggy alert observer", observerIggySource, marker);
}
for (const marker of [
  "NotApplicableMemory",
  "NotApplicableOutboxLocal",
  "IggyBundled",
  "IggyExternal",
  "DlqDuplicateAlertRuntimePublisher::new(config.policy)",
  "publisher.mark_unavailable()",
]) {
  requireText("server alert observer", observerServerSource, marker);
}

requireText("rustok-iggy module list", lib, "pub mod dlq_duplicate_inspection;");
requireText("rustok-iggy module list", lib, "pub mod dlq_duplicate_alert_policy;");
requireText("rustok-iggy module list", lib, "pub mod dlq_duplicate_alert_runtime;");
requireText("rustok-iggy module list", lib, "pub mod dlq_duplicate_alert_observer;");
for (const exportName of expectedExports) {
  requireText("rustok-iggy public exports", lib, exportName);
}

for (const marker of [
  "Failure kind, retry count, time, process identity, and random values are excluded",
  "pub fn delivery_id(&self) -> Uuid",
  "hash_part(&mut hasher, &self.raw_payload);",
  ".with_broker_message_id(delivery_id)",
]) {
  requireText("raw poison deterministic identity", decodeFailure, marker);
}
for (const marker of [
  "if entry.broker_message_id().is_some()",
  "IggyDlqPublisher::connect",
  ".publish(&entry)",
]) {
  requireText("production deterministic Iggy publisher", transport, marker);
}
for (const marker of [
  "Bounded aggregate view of neutral poison-result progress",
  "The snapshot intentionally excludes delivery identifiers",
  "Inspection never claims, releases, publishes, acknowledges, deletes, or repairs",
  "pub struct ConsumerPoisonReceiptSummary",
  "pub async fn summarize(",
]) {
  requireText("count-only receipt inspector", receiptInspector, marker);
}

if (
  contract.production_relationship?.raw_poison_delivery_id !==
    "ConsumedContractDecodeFailure::delivery_id" ||
  contract.production_relationship?.dlq_broker_message_id !==
    "ConsumedContractDecodeFailure::to_dlq_entry" ||
  contract.production_relationship?.physical_publisher !==
    "IggyTransport::move_to_dlq" ||
  contract.production_relationship?.receipt_summary !==
    "ConsumerPoisonReceiptInspector" ||
  contract.production_relationship
    ?.receipt_and_physical_duplicate_summaries_are_independent !== true
) {
  fail("DLQ duplicate inspection production relationship drift");
}

const requiredRemaining = new Set([
  "retained_external_iggy_duplicate_scan_evidence",
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
  "Iggy DLQ duplicate inspection source verified: deterministic header identity, in-memory exact-byte digest comparison, count-only privacy boundary, bounded auto_commit=false scanning, explicit alert policy, single-writer latest-value runtime, and mode-aware Memory/OutboxLocal/OutboxIggy server observation are locked; retained execution and telemetry/health projection remain pending.",
);
