#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-rolling-window-source.json";
const sourcePath = "crates/rustok-iggy/src/dlq_duplicate_rolling_window.rs";
const classifierPath = "crates/rustok-iggy/src/dlq_duplicate_inspection.rs";
const libPath = "crates/rustok-iggy/src/lib.rs";
const documentationPath = "crates/rustok-iggy/docs/dlq-duplicate-rolling-window.md";
const profilesCheckpointPath =
  "crates/rustok-profiles/docs/poison-duplicate-rolling-window-checkpoint.md";
const verifierPath = "scripts/verify/verify-iggy-dlq-duplicate-rolling-window.mjs";

const expectedExports = [
  "DlqDuplicateRollingWindowPolicy",
  "DlqDuplicateRollingWindow",
  "DlqDuplicateRollingWindowSnapshot",
  "DlqDuplicateRollingWindowError",
];
const expectedSnapshotFields = [
  "summary",
  "retained_cycles",
  "retained_observations",
  "evicted_cycles",
  "history_truncated",
];
const expectedTests = [
  "invalid_policy_and_capacity_overflow_fail_closed",
  "ordinary_duplicate_split_across_cycles_is_detected",
  "identity_conflict_split_across_cycles_requires_manual_review",
  "oldest_complete_cycle_eviction_marks_history_truncated",
  "oversized_cycle_rejection_preserves_existing_state",
];

const contract = JSON.parse(readFileSync(resolve(repoRoot, contractPath), "utf8"));
const source = readFileSync(resolve(repoRoot, sourcePath), "utf8");
const classifier = readFileSync(resolve(repoRoot, classifierPath), "utf8");
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

if (
  contract.schema_version !== 1 ||
  contract.module !== "iggy" ||
  contract.packet !== "dlq-duplicate-rolling-window-source" ||
  contract.status !== "source_complete_scanner_integration_pending" ||
  contract.owner !== "rustok-iggy" ||
  contract.source !== sourcePath ||
  contract.classifier_source !== classifierPath ||
  contract.verifier !== verifierPath ||
  contract.documentation !== documentationPath ||
  contract.profiles_checkpoint !== profilesCheckpointPath ||
  contract.execution_status !== "source_not_run"
) {
  fail("DLQ duplicate rolling-window contract identity or status drift");
}
if (!same(contract.public_exports, expectedExports)) {
  fail("DLQ duplicate rolling-window public export allowlist drift");
}
if (!same(contract.snapshot_fields, expectedSnapshotFields)) {
  fail("DLQ duplicate rolling-window snapshot field allowlist drift");
}
if (!same(contract.required_source_tests, expectedTests)) {
  fail("DLQ duplicate rolling-window focused test allowlist drift");
}

if (
  contract.bounds?.max_cycles_upper_bound !== 128 ||
  contract.bounds?.total_observation_capacity_upper_bound !== 10000 ||
  contract.bounds?.max_cycles_positive !== true ||
  contract.bounds?.max_observations_per_cycle_positive !== true ||
  contract.bounds?.checked_product_required !== true ||
  contract.bounds?.production_defaults !== false
) {
  fail("DLQ duplicate rolling-window bound contract drift");
}
if (
  contract.cycle_semantics?.input !==
    "one_complete_scan_cycle_of_opaque_duplicate_observations" ||
  contract.cycle_semantics?.empty_cycle_valid !== true ||
  contract.cycle_semantics?.oversized_cycle_rejected_transactionally !== true ||
  contract.cycle_semantics?.oldest_complete_cycle_evicted_at_capacity !== true ||
  contract.cycle_semantics?.duplicate_relationship_retained_across_cycles_while_present !==
    true ||
  contract.cycle_semantics?.identity_conflict_retained_across_cycles_while_present !==
    true ||
  contract.cycle_semantics?.partial_cycle_eviction !== false
) {
  fail("DLQ duplicate rolling-window cycle semantics drift");
}
if (
  contract.truncation_semantics?.history_truncated_after_first_eviction !== true ||
  contract.truncation_semantics?.truncated_snapshot_is_complete_history !== false ||
  contract.truncation_semantics?.truncated_snapshot_is_current_tail !== false ||
  contract.truncation_semantics?.evicted_identity_relationship_can_be_lost !== true
) {
  fail("DLQ duplicate rolling-window truncation semantics drift");
}
for (const [operation, allowed] of Object.entries(contract.mutation_boundary ?? {})) {
  if (allowed !== false) fail(`rolling-window external operation became allowed: ${operation}`);
}
if (
  contract.privacy_boundary?.observations_not_exported_from_state !== true ||
  contract.privacy_boundary?.identifier_free_snapshot !== true ||
  contract.privacy_boundary?.payload_free_snapshot !== true
) {
  fail("DLQ duplicate rolling-window privacy flags drift");
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
for (const field of contract.privacy_boundary?.snapshot_excludes ?? []) {
  requiredExcluded.delete(field);
}
if (requiredExcluded.size > 0) {
  fail(`rolling-window privacy exclusions are incomplete: ${[...requiredExcluded].join(", ")}`);
}

if (
  !same(contract.stable_errors, {
    invalid_policy: "iggy.dlq_duplicate.rolling_window_policy_invalid",
    cycle_too_large: "iggy.dlq_duplicate.rolling_window_cycle_too_large",
    count_overflow: "iggy.dlq_duplicate.rolling_window_count_overflow",
  })
) {
  fail("DLQ duplicate rolling-window stable error codes drift");
}

for (const marker of [
  "const MAX_ROLLING_WINDOW_CYCLES: u32 = 128;",
  "const MAX_ROLLING_WINDOW_OBSERVATIONS: u32 = 10_000;",
  "pub struct DlqDuplicateRollingWindowPolicy",
  ".checked_mul(max_observations_per_cycle)",
  ".filter(|total| *total <= MAX_ROLLING_WINDOW_OBSERVATIONS)",
  "pub struct DlqDuplicateRollingWindowSnapshot",
  "pub const fn history_truncated(&self) -> bool",
  "pub struct DlqDuplicateRollingWindow {",
  "cycles: VecDeque<Vec<DlqDuplicateObservation>>",
  "pub fn push_cycle(",
  "let mut candidate_cycles = self.cycles.clone();",
  "candidate_cycles.pop_front()",
  "candidate_cycles.push_back(incoming);",
  "self.cycles = candidate_cycles;",
  "flat_map(|cycle| cycle.iter().cloned())",
  "history_truncated: evicted_cycles > 0",
  "pub enum DlqDuplicateRollingWindowError",
  'Self::InvalidPolicy => "iggy.dlq_duplicate.rolling_window_policy_invalid"',
  'Self::CycleTooLarge => "iggy.dlq_duplicate.rolling_window_cycle_too_large"',
  'Self::CountOverflow => "iggy.dlq_duplicate.rolling_window_count_overflow"',
]) {
  requireText("DLQ duplicate rolling-window source", source, marker);
}
for (const testName of expectedTests) {
  requireText("DLQ duplicate rolling-window tests", source, `fn ${testName}()`);
}
if (countText(source, "#[test]") !== expectedTests.length) {
  fail("DLQ duplicate rolling-window source must contain exactly five focused unit tests");
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
  ".delete(",
  ".purge(",
  ".replay(",
  ".retry_entry(",
  ".reserve_and_claim(",
  ".mark_published(",
  "pub cycles:",
  "pub broker_message_id:",
  "pub payload_sha256:",
]) {
  forbidText("DLQ duplicate rolling-window source", source, marker);
}

for (const marker of [
  "pub struct DlqDuplicateObservation",
  "pub struct DlqDuplicateSummary",
  "pub fn summarize_dlq_duplicates(",
  "same exact bytes are ordinary physical copies",
]) {
  requireText("DLQ duplicate classifier", classifier, marker);
}
requireText(
  "rustok-iggy module list",
  lib,
  "pub mod dlq_duplicate_rolling_window;",
);
for (const exportName of expectedExports) {
  requireText("rustok-iggy public exports", lib, exportName);
}

for (const marker of [
  "complete scan cycles",
  "history_truncated",
  "does not move a broker cursor",
  "not complete history",
  "scanner integration remains pending",
]) {
  requireText("DLQ duplicate rolling-window documentation", documentation, marker);
}
for (const marker of [
  "Profiles never authorizes",
  "cross-cycle",
  "history_truncated",
  "per-partition cursor",
  "source-complete",
]) {
  requireText("Profiles rolling-window checkpoint", profilesCheckpoint, marker);
}

const requiredRemaining = new Set([
  "feed_complete_scanner_cycles_without_identifier_export",
  "define_per_partition_cursor_advancement",
  "define_state_persistence_or_restart_reset_semantics",
  "compose_mode_aware_server_observer",
  "retain_external_iggy_cross_cycle_runtime_evidence",
  "define_identifier_free_telemetry_and_health_projection",
]);
for (const item of contract.remaining_work ?? []) requiredRemaining.delete(item);
if (requiredRemaining.size > 0) {
  fail(`DLQ duplicate rolling-window remaining work drift: ${[
    ...requiredRemaining,
  ].join(", ")}`);
}

if (failures.length > 0) {
  console.error("Iggy DLQ duplicate rolling-window verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Iggy DLQ duplicate rolling-window source verified: explicit checked memory bounds, complete-cycle retention and eviction, transactional oversized-cycle rejection, cross-cycle ordinary/conflicting duplicate classification, identifier-free truncation metadata, no broker/receipt access, and explicit moving-cursor/current-tail/complete-history non-claims are locked; scanner, cursor, persistence, server, and runtime evidence integration remain pending.",
);
