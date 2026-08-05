#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-runtime-source.json";
const sourcePath = "crates/rustok-iggy/src/dlq_duplicate_alert_runtime.rs";
const policyPath = "crates/rustok-iggy/src/dlq_duplicate_alert_policy.rs";
const summaryPath = "crates/rustok-iggy/src/dlq_duplicate_inspection.rs";
const observerContractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-server-observer-source.json";
const observerIggySourcePath =
  "crates/rustok-iggy/src/dlq_duplicate_alert_observer.rs";
const observerServerSourcePath =
  "apps/server/src/services/event_dlq_duplicate_alert_observer.rs";
const libPath = "crates/rustok-iggy/src/lib.rs";
const expectedVerifier = "scripts/verify/verify-iggy-dlq-duplicate-alert-runtime.mjs";
const expectedDocumentation = "crates/rustok-iggy/docs/dlq-duplicate-alert-runtime.md";
const expectedProfilesCheckpoint =
  "crates/rustok-profiles/docs/poison-duplicate-alert-runtime-checkpoint.md";
const expectedExports = [
  "DlqDuplicateAlertRuntimePublisher",
  "DlqDuplicateAlertRuntimeSubscriber",
  "DlqDuplicateAlertRuntimeSnapshot",
  "DlqDuplicateAlertRuntimeError",
];
const expectedTests = [
  "initial_snapshot_is_unavailable_without_evaluation",
  "publish_replaces_latest_identifier_free_evaluation",
  "unavailable_transition_clears_stale_evaluation",
  "independent_subscribers_receive_the_same_latest_snapshot",
  "closed_publisher_has_a_stable_identifier_free_error",
];

const contract = JSON.parse(readFileSync(resolve(repoRoot, contractPath), "utf8"));
const observerContract = JSON.parse(
  readFileSync(resolve(repoRoot, observerContractPath), "utf8"),
);
const source = readFileSync(resolve(repoRoot, sourcePath), "utf8");
const policy = readFileSync(resolve(repoRoot, policyPath), "utf8");
const summary = readFileSync(resolve(repoRoot, summaryPath), "utf8");
const observerIggySource = readFileSync(
  resolve(repoRoot, observerIggySourcePath),
  "utf8",
);
const observerServerSource = readFileSync(
  resolve(repoRoot, observerServerSourcePath),
  "utf8",
);
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
  contract.packet !== "dlq-duplicate-alert-runtime-source" ||
  contract.status !== "source_complete_server_observer_execution_pending" ||
  contract.owner !== "rustok-iggy" ||
  contract.source !== sourcePath ||
  contract.policy_source !== policyPath ||
  contract.summary_source !== summaryPath ||
  contract.execution_status !== "source_not_run"
) {
  fail("DLQ duplicate alert runtime contract identity or source status drift");
}
if (!sameValue(contract.public_exports, expectedExports)) {
  fail("DLQ duplicate alert runtime public export allowlist drift");
}
if (!sameValue(contract.required_source_tests, expectedTests)) {
  fail("DLQ duplicate alert runtime focused test allowlist drift");
}
if (
  contract.verifier !== expectedVerifier ||
  contract.documentation !== expectedDocumentation ||
  contract.profiles_checkpoint !== expectedProfilesCheckpoint
) {
  fail("DLQ duplicate alert runtime verifier or documentation path drift");
}

if (
  contract.composition?.input !== "already_observed_DlqDuplicateSummary" ||
  contract.composition?.policy !== "prevalidated_DlqDuplicateAlertPolicy" ||
  contract.composition?.channel !== "tokio_watch_latest_value" ||
  contract.composition?.publisher_role !== "single_writer" ||
  contract.composition?.subscriber_role !== "read_only" ||
  contract.composition?.broker_scan !== false ||
  contract.composition?.receipt_read !== false
) {
  fail("DLQ duplicate alert runtime composition boundary drift");
}
if (
  !sameValue(contract.snapshot?.fields, ["generation", "available", "evaluation"]) ||
  contract.snapshot?.initial_generation !== 0 ||
  contract.snapshot?.initial_available !== false ||
  contract.snapshot?.initial_evaluation !== null ||
  contract.snapshot?.generation_monotonic !== true ||
  contract.snapshot?.generation_overflow_fails_closed !== true ||
  contract.snapshot?.available_snapshot_requires_evaluation !== true ||
  contract.snapshot?.unavailable_snapshot_clears_evaluation !== true ||
  contract.snapshot?.latest_value_replaces_prior_snapshot !== true
) {
  fail("DLQ duplicate alert runtime snapshot semantics drift");
}
if (
  contract.subscriber?.current_snapshot !== true ||
  contract.subscriber?.await_change !== true ||
  contract.subscriber?.independent_subscriptions !== true ||
  contract.subscriber?.publisher_closed_fails_bounded !== true ||
  contract.subscriber?.write_access !== false
) {
  fail("DLQ duplicate alert runtime subscriber boundary drift");
}

for (const [operation, allowed] of Object.entries(contract.runtime_boundary ?? {})) {
  if (allowed !== false) fail(`DLQ duplicate alert runtime operation became enabled: ${operation}`);
}
for (const [operation, allowed] of Object.entries(contract.mutation_boundary ?? {})) {
  if (allowed !== false) fail(`DLQ duplicate alert runtime mutation became enabled: ${operation}`);
}

if (
  contract.server_observer?.status !== "source_complete_execution_pending" ||
  contract.server_observer?.contract !== observerContractPath ||
  contract.server_observer?.iggy_source !== observerIggySourcePath ||
  contract.server_observer?.server_source !== observerServerSourcePath ||
  !sameValue(contract.server_observer?.delivery_profiles, [
    "outbox_local_not_applicable",
    "outbox_iggy_bundled",
    "outbox_iggy_external",
  ]) ||
  contract.server_observer?.readiness_dependency !== false ||
  contract.server_observer?.profiles_authorization !== false ||
  observerContract.packet !== "dlq-duplicate-alert-server-observer-source" ||
  observerContract.status !== "source_complete_runtime_execution_pending" ||
  observerContract.iggy_source !== observerIggySourcePath ||
  observerContract.server_source !== observerServerSourcePath
) {
  fail("DLQ duplicate alert server observer relationship drift");
}

for (const marker of [
  "use tokio::sync::watch;",
  "pub struct DlqDuplicateAlertRuntimeSnapshot",
  "generation: u64",
  "available: bool",
  "evaluation: Option<DlqDuplicateAlertEvaluation>",
  "const fn unavailable(generation: u64)",
  "evaluation: None",
  "const fn available(",
  "evaluation: Some(evaluation)",
  "pub struct DlqDuplicateAlertRuntimePublisher",
  "policy: DlqDuplicateAlertPolicy",
  "generation: u64",
  "sender: watch::Sender<DlqDuplicateAlertRuntimeSnapshot>",
  "watch::channel(DlqDuplicateAlertRuntimeSnapshot::unavailable(0))",
  "pub fn subscribe(&self)",
  "pub fn publish(\n        &mut self,",
  "self.policy.evaluate(summary)",
  "self.sender.send_replace(snapshot);",
  "pub fn mark_unavailable(\n        &mut self,",
  "DlqDuplicateAlertRuntimeSnapshot::unavailable(self.advance_generation()?)",
  "fn advance_generation(&mut self)",
  ".checked_add(1)",
  "pub struct DlqDuplicateAlertRuntimeSubscriber",
  "receiver: watch::Receiver<DlqDuplicateAlertRuntimeSnapshot>",
  "pub fn current(&self)",
  "pub async fn changed(",
  ".map_err(|_| DlqDuplicateAlertRuntimeError::PublisherClosed)?;",
  "pub enum DlqDuplicateAlertRuntimeError",
  '"iggy.dlq_duplicate.alert_runtime_generation_overflow"',
  '"iggy.dlq_duplicate.alert_runtime_publisher_closed"',
]) {
  requireText("DLQ duplicate alert runtime source", source, marker);
}
for (const testName of expectedTests) {
  requireText("DLQ duplicate alert runtime source tests", source, `fn ${testName}()`);
}
if (countText(source, "#[test]") + countText(source, "#[tokio::test]") !== expectedTests.length) {
  fail("DLQ duplicate alert runtime source must contain exactly five focused tests");
}

for (const marker of [
  "Serialize",
  "Deserialize",
  "tracing::",
  "println!(",
  "eprintln!(",
  "ServerRuntimeContext",
  "consumer_poison_metrics",
  "runtime_consumer_metrics",
  "IggyClient",
  "IggyTransport",
  "ConsumerPoisonReceiptInspector",
  ".poll_messages(",
  ".get_consumer_offset(",
  ".summarize(",
  ".move_to_dlq(",
  ".send_messages(",
  ".store_consumer_offset(",
  ".acknowledge(",
  ".delete_stream(",
  ".delete_topic(",
  ".purge_topic(",
  ".reserve_and_claim(",
  ".mark_published(",
  ".mark_acknowledged(",
  ".notify(",
  ".page(",
]) {
  forbidText("DLQ duplicate alert runtime source", source, marker);
}

for (const marker of [
  "pub struct DlqDuplicateAlertPolicy",
  "pub const fn evaluate(",
  "pub struct DlqDuplicateAlertEvaluation",
  "pub enum DlqDuplicateAlertLevel",
]) {
  requireText("DLQ duplicate alert policy source", policy, marker);
}
for (const marker of [
  "pub struct DlqDuplicateSummary",
  "pub const fn has_physical_duplicates(&self)",
  "pub const fn has_identity_conflicts(&self)",
]) {
  requireText("DLQ duplicate summary source", summary, marker);
}
for (const marker of [
  "pub struct IggyDlqDuplicateAlertObserver",
  "IggyDlqDuplicateScanner::new(client, stream_name)?",
]) {
  requireText("Iggy observer composition", observerIggySource, marker);
}
for (const marker of [
  "NotApplicableOutboxLocal",
  "IggyBundled",
  "IggyExternal",
  "DlqDuplicateAlertRuntimePublisher::new(config.policy)",
  "publisher.mark_unavailable()",
]) {
  requireText("server observer composition", observerServerSource, marker);
}

requireText("rustok-iggy module list", lib, "pub mod dlq_duplicate_alert_runtime;");
for (const exportName of expectedExports) {
  requireText("rustok-iggy public exports", lib, exportName);
}

const requiredPrivacyExclusions = new Set([
  "source_counts",
  "threshold_values",
  "broker_address",
  "stream",
  "topic",
  "partition",
  "offset",
  "broker_message_id",
  "payload",
  "payload_sha256",
  "receipt_identity",
  "error_classification",
  "publisher_identity",
  "timestamp",
  "credential",
  "raw_client_error",
]);
for (const field of contract.privacy_boundary?.snapshot_excludes ?? []) {
  requiredPrivacyExclusions.delete(field);
}
if (requiredPrivacyExclusions.size > 0) {
  fail(`DLQ duplicate alert runtime privacy exclusions are incomplete: ${[
    ...requiredPrivacyExclusions,
  ].join(", ")}`);
}
if (
  contract.privacy_boundary?.serialization_added !== false ||
  contract.privacy_boundary?.persistence_added !== false
) {
  fail("DLQ duplicate alert runtime privacy persistence boundary drift");
}

const requiredRemaining = new Set([
  "telemetry_and_health_projection",
  "retained_runtime_integration_evidence",
  "notification_delivery_and_suppression_outside_runtime",
  "authorized_destructive_reconciliation_workflow",
  "aggregate_receipt_and_duplicate_health_correlation",
]);
for (const item of contract.remaining_work ?? []) requiredRemaining.delete(item);
if (requiredRemaining.size > 0) {
  fail(`DLQ duplicate alert runtime remaining work drift: ${[...requiredRemaining].join(", ")}`);
}

if (failures.length > 0) {
  console.error("Iggy DLQ duplicate alert runtime verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Iggy DLQ duplicate alert runtime source verified: single-writer monotonic latest-value publication, initial/unavailable stale-clearing semantics, read-only independent subscribers, identifier-free stable errors, no broker/receipt/Profile mutation, and the explicit OutboxLocal/OutboxIggy server observer relationship are locked; server execution and telemetry/health projection remain pending.",
);
