#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-iggy/contracts/evidence/dedup-recovery-window-policy-source.json";
const executionContractPath =
  "crates/rustok-iggy/contracts/evidence/dedup-recovery-window-calibration-execution-contract.json";
const sourcePath = "crates/rustok-iggy/src/dedup_recovery_window_policy.rs";
const testPath = "crates/rustok-iggy/tests/dedup_recovery_window_calibration.rs";
const libPath = "crates/rustok-iggy/src/lib.rs";
const documentationPath = "crates/rustok-iggy/docs/dedup-recovery-window-policy.md";
const profilesCheckpointPath =
  "crates/rustok-profiles/docs/poison-dedup-recovery-window-checkpoint.md";
const verifierPath = "scripts/verify/verify-iggy-dedup-recovery-window-policy.mjs";
const runnerPath =
  "scripts/evidence/capture-iggy-dedup-recovery-window-calibration.mjs";
const retainedVerifierPath =
  "scripts/verify/verify-iggy-dedup-recovery-window-retained.mjs";
const evidencePath =
  "crates/rustok-iggy/contracts/evidence/dedup-recovery-window-calibration-execution.json";

const expectedExports = [
  "IggyDeduplicationConfiguration",
  "IggyDedupRecoveryWindowPolicy",
  "IggyDedupRecoveryWindowAssessment",
  "IggyDedupRecoveryWindowStatus",
  "IggyDedupRecoveryWindowPolicyError",
];
const expectedHorizonInputs = [
  "publication_lease",
  "process_restart",
  "transport_reconnect",
  "operator_recovery",
];
const expectedStatuses = [
  "disabled",
  "insufficient_expiry",
  "insufficient_capacity",
  "insufficient_expiry_and_capacity",
  "sufficient",
];
const expectedTests = [
  "invalid_policy_and_configuration_fail_closed",
  "recovery_horizon_overflow_fails_closed",
  "disabled_configuration_never_claims_sufficiency",
  "expiry_and_capacity_deficits_are_distinguished",
  "exact_boundary_is_sufficient_without_stronger_guarantees",
];

const contract = JSON.parse(readFileSync(resolve(repoRoot, contractPath), "utf8"));
const executionContract = JSON.parse(
  readFileSync(resolve(repoRoot, executionContractPath), "utf8"),
);
const source = readFileSync(resolve(repoRoot, sourcePath), "utf8");
const test = readFileSync(resolve(repoRoot, testPath), "utf8");
const lib = readFileSync(resolve(repoRoot, libPath), "utf8");
const documentation = readFileSync(resolve(repoRoot, documentationPath), "utf8");
const profilesCheckpoint = readFileSync(
  resolve(repoRoot, profilesCheckpointPath),
  "utf8",
);
const runner = readFileSync(resolve(repoRoot, runnerPath), "utf8");
const retainedVerifier = readFileSync(resolve(repoRoot, retainedVerifierPath), "utf8");
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
  contract.schema_version !== 2 ||
  contract.module !== "iggy" ||
  contract.packet !== "dedup-recovery-window-policy-source" ||
  contract.status !== "source_complete_retained_calibration_pending" ||
  contract.owner !== "rustok-iggy" ||
  contract.source !== sourcePath ||
  contract.verifier !== verifierPath ||
  contract.documentation !== documentationPath ||
  contract.profiles_checkpoint !== profilesCheckpointPath ||
  contract.execution_status !== "source_not_run"
) {
  fail("dedup recovery-window contract identity or source status drift");
}

if (!sameValue(contract.public_exports, expectedExports)) {
  fail("dedup recovery-window public export allowlist drift");
}
if (!sameValue(contract.required_recovery_horizon_inputs, expectedHorizonInputs)) {
  fail("dedup recovery-window horizon input allowlist drift");
}
if (!sameValue(contract.assessment_statuses, expectedStatuses)) {
  fail("dedup recovery-window status allowlist drift");
}
if (!sameValue(contract.required_source_tests, expectedTests)) {
  fail("dedup recovery-window focused unit-test allowlist drift");
}
if (
  contract.required_capacity_input !==
    "maximum_distinct_deterministic_message_ids_per_partition_during_recovery_horizon" ||
  contract.expiry_rule !== "checked_sum_of_all_recovery_horizon_inputs" ||
  contract.capacity_rule !==
    "configured_max_entries_not_below_required_per_partition_distinct_ids"
) {
  fail("dedup recovery-window comparison rules drift");
}

if (
  !sameValue(contract.stable_codes, {
    disabled: "iggy.dedup_recovery.disabled",
    insufficient_expiry: "iggy.dedup_recovery.insufficient_expiry",
    insufficient_capacity: "iggy.dedup_recovery.insufficient_capacity",
    insufficient_expiry_and_capacity:
      "iggy.dedup_recovery.insufficient_expiry_and_capacity",
    sufficient: "iggy.dedup_recovery.sufficient",
    invalid_policy: "iggy.dedup_recovery.policy_invalid",
    invalid_configuration: "iggy.dedup_recovery.configuration_invalid",
    horizon_overflow: "iggy.dedup_recovery.horizon_overflow",
  })
) {
  fail("dedup recovery-window stable code contract drift");
}

for (const [operation, allowed] of Object.entries(contract.policy_boundary ?? {})) {
  if (allowed !== false) fail(`dedup recovery-window boundary became allowed: ${operation}`);
}
for (const [field, required] of Object.entries(contract.privacy_boundary ?? {})) {
  if (required !== true) fail(`dedup recovery-window privacy boundary weakened: ${field}`);
}

const retained = contract.retained_calibration;
if (
  retained?.status !== "capture_source_complete_execution_pending" ||
  retained?.contract !== executionContractPath ||
  retained?.test !== testPath ||
  retained?.runner !== runnerPath ||
  retained?.verifier !== retainedVerifierPath ||
  retained?.evidence_path !== evidencePath ||
  retained?.canonical_packet_present !== false ||
  retained?.requires_reviewed_bounds_file !== true ||
  retained?.requires_reviewed_enabled_configuration !== true ||
  retained?.requires_sufficient_assessment !== true ||
  retained?.no_clobber_write !== true
) {
  fail("dedup recovery-window retained calibration relationship drift");
}
if (
  executionContract.packet !==
    "dedup-recovery-window-calibration-execution-contract" ||
  executionContract.status !== "runtime_execution_contract_locked" ||
  executionContract.source_contract !== contractPath ||
  executionContract.test_target !== "dedup_recovery_window_calibration" ||
  executionContract.case !== "reviewed_configuration_covers_recovery_window" ||
  executionContract.runner !== runnerPath ||
  executionContract.verifier !== retainedVerifierPath ||
  executionContract.source_verifier !== verifierPath ||
  executionContract.evidence_path !== evidencePath ||
  executionContract.evidence_status !== "runtime_calibration_pending"
) {
  fail("dedup recovery-window execution contract relationship drift");
}

for (const marker of [
  "pub enum IggyDeduplicationConfiguration",
  "Disabled,",
  "Enabled { max_entries: u64, expiry: Duration },",
  "max_entries == 0 || expiry.is_zero()",
  "pub struct IggyDedupRecoveryWindowPolicy",
  "publication_lease: Duration",
  "process_restart: Duration",
  "transport_reconnect: Duration",
  "operator_recovery: Duration",
  "required_max_entries_per_partition: u64",
  ".checked_add(process_restart)",
  ".and_then(|value| value.checked_add(transport_reconnect))",
  ".and_then(|value| value.checked_add(operator_recovery))",
  "pub fn assess(",
  "let expiry_sufficient = expiry >= self.required_expiry;",
  "let capacity_sufficient = max_entries >= self.required_max_entries_per_partition;",
  "pub struct IggyDedupRecoveryWindowAssessment",
  "pub enum IggyDedupRecoveryWindowStatus",
  "InsufficientExpiryAndCapacity",
  "pub const fn is_sufficient(&self) -> bool",
  "pub const fn requires_operator_action(&self) -> bool",
  'Self::Sufficient => "iggy.dedup_recovery.sufficient"',
  "pub enum IggyDedupRecoveryWindowPolicyError",
  'Self::InvalidPolicy => "iggy.dedup_recovery.policy_invalid"',
  'Self::InvalidConfiguration => "iggy.dedup_recovery.configuration_invalid"',
  'Self::RecoveryHorizonOverflow => "iggy.dedup_recovery.horizon_overflow"',
]) {
  requireText("dedup recovery-window policy source", source, marker);
}

for (const testName of expectedTests) {
  requireText("dedup recovery-window policy tests", source, `fn ${testName}()`);
}
if (countText(source, "#[test]") !== expectedTests.length) {
  fail("dedup recovery-window policy source must contain exactly five focused unit tests");
}

for (const marker of [
  "IggyClient",
  "IggyTransport",
  "ConsumerPoisonReceipt",
  "Serialize",
  "Deserialize",
  ".connect(",
  ".poll_messages(",
  ".move_to_dlq(",
  ".acknowledge(",
  ".reserve_and_claim(",
  ".mark_published(",
  ".mark_acknowledged(",
  ".delete(",
  ".purge(",
  ".replay(",
]) {
  forbidText("dedup recovery-window policy source", source, marker);
}

for (const marker of [
  "const SKIP_MESSAGE",
  "fn reviewed_configuration_covers_recovery_window()",
  "IggyDedupRecoveryWindowPolicy::new(",
  "IggyDeduplicationConfiguration::enabled(",
  "IggyDedupRecoveryWindowStatus::Sufficient",
  "RUSTOK_DEDUP_RECOVERY_CALIBRATION status={}",
]) {
  requireText("dedup recovery-window retained calibration test", test, marker);
}
if (countText(test, "#[test]") !== 1) {
  fail("dedup recovery-window retained calibration must contain exactly one focused test");
}
for (const marker of [
  "IggyClient",
  "IggyTransport",
  ".connect(",
  ".poll_messages(",
  ".move_to_dlq(",
  ".acknowledge(",
  ".delete(",
  ".purge(",
]) {
  forbidText("dedup recovery-window retained calibration test", test, marker);
}

requireText(
  "rustok-iggy module list",
  lib,
  "pub mod dedup_recovery_window_policy;",
);
for (const exportName of expectedExports) {
  requireText("rustok-iggy public exports", lib, exportName);
}

for (const marker of [
  "checked sum",
  "per physical partition",
  "No production default",
  "does not prove exactly-once",
  "Retained calibration",
  "canonical packet remains pending",
]) {
  requireText("dedup recovery-window documentation", documentation, marker);
}
for (const marker of [
  "Profiles never authorizes",
  "source-complete",
  "retained calibration",
  "no-clobber",
  "multi-replica",
]) {
  requireText("Profiles dedup recovery-window checkpoint", profilesCheckpoint, marker);
}
for (const marker of [
  "function reviewedBounds(",
  "function reviewedConfiguration(",
  "function ensureCleanCommit(",
  "function parsePassedAssessment(",
  "function writeNoClobber(",
  "linkSync(temporaryPath, outputPath)",
  "sourceHashes()",
  "reported a skip",
]) {
  requireText("dedup recovery-window capture runner", runner, marker);
}
for (const marker of [
  "canonical runtime packet is pending",
  "canonical recovery-window source hash is stale",
  "canonical sufficient recovery-window assessment drift",
  "forbidden field",
]) {
  requireText("dedup recovery-window retained verifier", retainedVerifier, marker);
}

const requiredRemainingWork = new Set([
  "execute_retained_calibration_on_reviewed_production_inputs",
  "review_capacity_basis_and_recovery_horizon_bounds",
  "commit_no_clobber_calibration_packet",
  "repeat_when_bound_source_configuration_or_bounds_change",
  "repeat_for_bundled_tls_auth_failover_and_multi_replica_operation",
]);
for (const item of contract.remaining_work ?? []) requiredRemainingWork.delete(item);
if (requiredRemainingWork.size > 0) {
  fail(`dedup recovery-window remaining work drift: ${[
    ...requiredRemainingWork,
  ].join(", ")}`);
}

if (failures.length > 0) {
  console.error("Iggy dedup recovery-window policy verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Iggy dedup recovery-window policy source verified: reviewed disabled/enabled configuration, checked additive recovery horizon, explicit per-partition capacity bound, fail-closed assessments, stable codes, identifier-free projection, exact retained calibration test, reviewed bounds/config digests, clean-commit source binding, skip rejection, sufficient-only gate, no-clobber publication, no broker or receipt access, no production defaults, and no exactly-once claim are locked; runtime calibration and canonical retained evidence remain pending.",
);
