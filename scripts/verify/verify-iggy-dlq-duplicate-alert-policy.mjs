#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-policy-source.json";
const runtimeContractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-runtime-source.json";
const sourcePath = "crates/rustok-iggy/src/dlq_duplicate_alert_policy.rs";
const runtimeSourcePath = "crates/rustok-iggy/src/dlq_duplicate_alert_runtime.rs";
const summaryPath = "crates/rustok-iggy/src/dlq_duplicate_inspection.rs";
const libPath = "crates/rustok-iggy/src/lib.rs";
const expectedVerifier = "scripts/verify/verify-iggy-dlq-duplicate-alert-policy.mjs";
const expectedDocumentation = "crates/rustok-iggy/docs/dlq-duplicate-alert-policy.md";
const expectedProfilesCheckpoint =
  "crates/rustok-profiles/docs/poison-duplicate-alert-policy-checkpoint.md";
const expectedExports = [
  "DlqDuplicateAlertPolicy",
  "DlqDuplicateAlertLevel",
  "DlqDuplicateAlertEvaluation",
  "DlqDuplicateAlertPolicyError",
];
const expectedLevels = ["clear", "notice", "warning", "critical"];
const expectedDimensions = [
  "duplicate_messages",
  "duplicate_groups",
  "max_copies_per_message_id",
  "identity_conflict",
];
const expectedProjection = [
  "level",
  "physical_duplicates",
  "identity_conflict",
  "duplicate_messages_threshold_reached",
  "duplicate_groups_threshold_reached",
  "max_copies_threshold_reached",
];
const expectedTests = [
  "invalid_threshold_ordering_fails_closed",
  "clear_and_notice_remain_below_operator_thresholds",
  "warning_reports_only_reached_warning_dimensions",
  "critical_numeric_threshold_takes_precedence",
  "identity_conflict_is_always_critical_and_manual",
];

const contract = JSON.parse(readFileSync(resolve(repoRoot, contractPath), "utf8"));
const runtimeContract = JSON.parse(
  readFileSync(resolve(repoRoot, runtimeContractPath), "utf8"),
);
const source = readFileSync(resolve(repoRoot, sourcePath), "utf8");
const runtimeSource = readFileSync(resolve(repoRoot, runtimeSourcePath), "utf8");
const summary = readFileSync(resolve(repoRoot, summaryPath), "utf8");
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
  contract.packet !== "dlq-duplicate-alert-policy-source" ||
  contract.status !== "source_complete_runtime_composition_pending_server_integration" ||
  contract.owner !== "rustok-iggy" ||
  contract.source !== sourcePath ||
  contract.summary_source !== summaryPath ||
  contract.execution_status !== "source_not_run"
) {
  fail("DLQ duplicate alert policy contract identity or source status drift");
}
if (!sameValue(contract.public_exports, expectedExports)) {
  fail("DLQ duplicate alert policy public export allowlist drift");
}
if (!sameValue(contract.levels, expectedLevels)) {
  fail("DLQ duplicate alert level allowlist drift");
}
if (!sameValue(contract.evaluated_dimensions, expectedDimensions)) {
  fail("DLQ duplicate alert evaluated dimension allowlist drift");
}
if (!sameValue(contract.evaluation_projection, expectedProjection)) {
  fail("DLQ duplicate alert evaluation projection drift");
}
if (!sameValue(contract.required_source_tests, expectedTests)) {
  fail("DLQ duplicate alert policy source test allowlist drift");
}
if (
  contract.verifier !== expectedVerifier ||
  contract.documentation !== expectedDocumentation ||
  contract.profiles_checkpoint !== expectedProfilesCheckpoint
) {
  fail("DLQ duplicate alert policy verifier or documentation path drift");
}

if (
  contract.thresholds?.caller_must_supply_all !== true ||
  contract.thresholds?.production_defaults !== false ||
  contract.thresholds?.warning_duplicate_messages_minimum !== 1 ||
  contract.thresholds?.critical_duplicate_messages_not_below_warning !== true ||
  contract.thresholds?.warning_duplicate_groups_minimum !== 1 ||
  contract.thresholds?.critical_duplicate_groups_not_below_warning !== true ||
  contract.thresholds?.warning_max_copies_per_message_id_minimum !== 2 ||
  contract.thresholds?.critical_max_copies_not_below_warning !== true ||
  contract.thresholds?.invalid_thresholds_fail_closed !== true
) {
  fail("DLQ duplicate alert threshold contract drift");
}
if (
  !sameValue(contract.precedence, [
    "identity_conflict_is_always_critical",
    "critical_numeric_threshold",
    "warning_numeric_threshold",
    "physical_duplicate_below_threshold_is_notice",
    "no_physical_duplicate_is_clear",
  ])
) {
  fail("DLQ duplicate alert precedence drift");
}
if (
  contract.manual_review?.identity_conflict !== true ||
  contract.manual_review?.numeric_critical_without_identity_conflict !== false ||
  contract.manual_review?.destructive_action !== false
) {
  fail("DLQ duplicate alert manual-review boundary drift");
}
if (
  contract.privacy_boundary?.input_is_count_only_summary !== true ||
  contract.privacy_boundary?.serialization_added !== false
) {
  fail("DLQ duplicate alert privacy boundary drift");
}
for (const [operation, allowed] of Object.entries(contract.policy_boundary ?? {})) {
  if (allowed !== false) fail(`DLQ duplicate alert external policy became allowed: ${operation}`);
}
for (const [operation, allowed] of Object.entries(contract.mutation_boundary ?? {})) {
  if (allowed !== false) fail(`DLQ duplicate alert mutation became allowed: ${operation}`);
}

if (
  contract.runtime_composition?.status !== "source_complete_server_integration_pending" ||
  contract.runtime_composition?.contract !== runtimeContractPath ||
  contract.runtime_composition?.source !== runtimeSourcePath ||
  contract.runtime_composition?.input !== "DlqDuplicateSummary" ||
  contract.runtime_composition?.output !== "DlqDuplicateAlertRuntimeSnapshot" ||
  contract.runtime_composition?.latest_value !== true ||
  contract.runtime_composition?.single_writer !== true ||
  contract.runtime_composition?.unavailable_clears_evaluation !== true ||
  runtimeContract.packet !== "dlq-duplicate-alert-runtime-source" ||
  runtimeContract.status !== "source_complete_server_integration_pending" ||
  runtimeContract.source !== runtimeSourcePath ||
  runtimeContract.policy_source !== sourcePath ||
  runtimeContract.summary_source !== summaryPath
) {
  fail("DLQ duplicate alert runtime composition relationship drift");
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
  "raw_threshold_values",
]);
for (const field of contract.privacy_boundary?.evaluation_excludes ?? []) {
  requiredExcludedFields.delete(field);
}
if (requiredExcludedFields.size > 0) {
  fail(`DLQ duplicate alert privacy exclusions are incomplete: ${[...requiredExcludedFields].join(", ")}`);
}

if (
  !sameValue(contract.stable_codes, {
    clear: "iggy.dlq_duplicate.alert.clear",
    notice: "iggy.dlq_duplicate.alert.notice",
    warning: "iggy.dlq_duplicate.alert.warning",
    critical: "iggy.dlq_duplicate.alert.critical",
    invalid_policy: "iggy.dlq_duplicate.alert_policy_invalid",
  })
) {
  fail("DLQ duplicate alert stable code contract drift");
}

for (const marker of [
  "pub struct DlqDuplicateAlertPolicy",
  "warning_duplicate_messages: u64",
  "critical_duplicate_messages: u64",
  "warning_duplicate_groups: u64",
  "critical_duplicate_groups: u64",
  "warning_max_copies_per_message_id: u64",
  "critical_max_copies_per_message_id: u64",
  "pub fn new(",
  "warning_duplicate_messages == 0",
  "warning_duplicate_groups == 0",
  "warning_max_copies_per_message_id < 2",
  "critical_duplicate_messages < warning_duplicate_messages",
  "critical_duplicate_groups < warning_duplicate_groups",
  "critical_max_copies_per_message_id < warning_max_copies_per_message_id",
  "pub const fn evaluate(",
  "summary: &DlqDuplicateSummary",
  "let identity_conflict = summary.has_identity_conflicts();",
  "DlqDuplicateAlertLevel::Critical",
  "DlqDuplicateAlertLevel::Warning",
  "DlqDuplicateAlertLevel::Notice",
  "DlqDuplicateAlertLevel::Clear",
  "pub enum DlqDuplicateAlertLevel",
  'Self::Clear => "iggy.dlq_duplicate.alert.clear"',
  'Self::Notice => "iggy.dlq_duplicate.alert.notice"',
  'Self::Warning => "iggy.dlq_duplicate.alert.warning"',
  'Self::Critical => "iggy.dlq_duplicate.alert.critical"',
  "pub struct DlqDuplicateAlertEvaluation",
  "level: DlqDuplicateAlertLevel",
  "physical_duplicates: bool",
  "identity_conflict: bool",
  "duplicate_messages_threshold_reached: bool",
  "duplicate_groups_threshold_reached: bool",
  "max_copies_threshold_reached: bool",
  "pub const fn requires_manual_review(&self) -> bool",
  "pub enum DlqDuplicateAlertPolicyError",
  'Self::InvalidThresholds => "iggy.dlq_duplicate.alert_policy_invalid"',
]) {
  requireText("DLQ duplicate alert policy source", source, marker);
}

for (const testName of expectedTests) {
  requireText("DLQ duplicate alert policy tests", source, `fn ${testName}()`);
}
if (countText(source, "#[test]") !== expectedTests.length) {
  fail("DLQ duplicate alert policy source must contain exactly five focused unit tests");
}

for (const marker of [
  "pub warning_duplicate_messages:",
  "pub critical_duplicate_messages:",
  "pub warning_duplicate_groups:",
  "pub critical_duplicate_groups:",
  "pub warning_max_copies_per_message_id:",
  "pub critical_max_copies_per_message_id:",
  "Serialize",
  "Deserialize",
  "tokio::sync::watch",
  "IggyClient",
  "IggyTransport",
  "ConsumerPoisonReceipt",
  ".poll_messages(",
  ".move_to_dlq(",
  ".acknowledge(",
  ".delete(",
  ".purge(",
  ".replay(",
  ".retry_entry(",
  ".reserve_and_claim(",
  ".release_claim(",
  ".mark_published(",
  ".mark_acknowledged(",
  ".send(",
  ".notify(",
  ".page(",
]) {
  forbidText("DLQ duplicate alert policy source", source, marker);
}

for (const marker of [
  "pub struct DlqDuplicateSummary",
  "pub const fn duplicate_messages(&self)",
  "pub const fn duplicate_groups(&self)",
  "pub const fn max_copies_per_message_id(&self)",
  "pub const fn has_physical_duplicates(&self)",
  "pub const fn has_identity_conflicts(&self)",
]) {
  requireText("count-only DLQ duplicate summary", summary, marker);
}
for (const marker of [
  "pub struct DlqDuplicateAlertRuntimePublisher",
  "pub struct DlqDuplicateAlertRuntimeSnapshot",
  "watch::channel(DlqDuplicateAlertRuntimeSnapshot::unavailable(0))",
  "pub fn publish(\n        &mut self,",
  "self.policy.evaluate(summary)",
  "pub fn mark_unavailable(\n        &mut self,",
  "evaluation: None",
]) {
  requireText("DLQ duplicate alert runtime source", runtimeSource, marker);
}

requireText("rustok-iggy module list", lib, "pub mod dlq_duplicate_alert_policy;");
for (const exportName of expectedExports) {
  requireText("rustok-iggy public exports", lib, exportName);
}

const requiredRemainingWork = new Set([
  "server_observer_integration",
  "telemetry_and_health_projection",
  "alert_delivery_and_suppression_outside_policy",
  "retained_policy_integration_evidence",
  "authorized_destructive_reconciliation_workflow",
  "aggregate_receipt_and_duplicate_health_correlation",
]);
for (const item of contract.remaining_work ?? []) requiredRemainingWork.delete(item);
if (requiredRemainingWork.size > 0) {
  fail(`DLQ duplicate alert remaining work drift: ${[...requiredRemainingWork].join(", ")}`);
}

if (failures.length > 0) {
  console.error("Iggy DLQ duplicate alert policy verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Iggy DLQ duplicate alert policy source verified: explicit monotonic thresholds, clear/notice/warning/critical precedence, conflict-critical manual escalation, count-only boolean projection, stable codes, no production defaults, no broker/receipt access, no notification dispatch, no destructive action, and the single-writer stale-clearing latest-value runtime composition are locked; server integration remains pending.",
);
