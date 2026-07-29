#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-iggy/contracts/evidence/dedup-recovery-window-policy-source.json";
const sourcePath = "crates/rustok-iggy/src/dedup_recovery_window_policy.rs";
const libPath = "crates/rustok-iggy/src/lib.rs";
const documentationPath = "crates/rustok-iggy/docs/dedup-recovery-window-policy.md";
const profilesCheckpointPath =
  "crates/rustok-profiles/docs/poison-dedup-recovery-window-checkpoint.md";
const verifierPath = "scripts/verify/verify-iggy-dedup-recovery-window-policy.mjs";

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
const source = readFileSync(resolve(repoRoot, sourcePath), "utf8");
const lib = readFileSync(resolve(repoRoot, libPath), "utf8");
const documentation = readFileSync(resolve(repoRoot, documentationPath), "utf8");
const profilesCheckpoint = readFileSync(
  resolve(repoRoot, profilesCheckpointPath),
  "utf8",
);
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
  contract.packet !== "dedup-recovery-window-policy-source" ||
  contract.status !== "source_complete_runtime_calibration_pending" ||
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
  "runtime calibration remains pending",
]) {
  requireText("dedup recovery-window documentation", documentation, marker);
}
for (const marker of [
  "Profiles never authorizes",
  "source-complete",
  "runtime calibration",
  "multi-replica",
]) {
  requireText("Profiles dedup recovery-window checkpoint", profilesCheckpoint, marker);
}

const requiredRemainingWork = new Set([
  "supply_reviewed_production_horizon_bounds",
  "derive_per_partition_distinct_id_capacity_bound",
  "bind_assessment_to_reviewed_iggy_configuration_digest",
  "retain_runtime_calibration_packet",
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
  "Iggy dedup recovery-window policy source verified: reviewed disabled/enabled configuration, checked additive recovery horizon, explicit per-partition capacity bound, fail-closed expiry/capacity assessments, stable codes, identifier-free projection, no broker or receipt access, no production defaults, and no exactly-once claim are locked; runtime calibration and retained evidence remain pending.",
);
