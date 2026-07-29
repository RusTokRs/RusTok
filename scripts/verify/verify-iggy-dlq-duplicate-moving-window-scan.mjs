#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-moving-window-scan-source.json";
const sourcePath = "crates/rustok-iggy/src/dlq_duplicate_moving_window_scan.rs";
const rollingPath = "crates/rustok-iggy/src/dlq_duplicate_rolling_window.rs";
const classifierPath = "crates/rustok-iggy/src/dlq_duplicate_inspection.rs";
const libPath = "crates/rustok-iggy/src/lib.rs";
const documentationPath =
  "crates/rustok-iggy/docs/dlq-duplicate-moving-window-scan.md";
const profilesCheckpointPath =
  "crates/rustok-profiles/docs/poison-duplicate-moving-window-scan-checkpoint.md";
const planPath = "crates/rustok-profiles/docs/implementation-plan.md";
const verifierPath =
  "scripts/verify/verify-iggy-dlq-duplicate-moving-window-scan.mjs";

const expectedExports = [
  "IggyDlqDuplicateMovingWindowPolicy",
  "IggyDlqDuplicateMovingWindowState",
  "IggyDlqDuplicateMovingWindowSnapshot",
  "IggyDlqDuplicateMovingWindowScanner",
  "IggyDlqDuplicateMovingWindowError",
];
const expectedTests = [
  "policy_requires_complete_fair_cycle_to_fit_rolling_capacity",
  "complete_cycle_advances_partition_cursors_independently",
  "duplicate_split_across_advancing_cycles_remains_count_only",
  "incomplete_cycle_preserves_cursors_and_rolling_state",
  "explicit_reset_rewinds_cursors_and_clears_rolling_history",
];

const contract = JSON.parse(readFileSync(resolve(repoRoot, contractPath), "utf8"));
const source = readFileSync(resolve(repoRoot, sourcePath), "utf8");
const rolling = readFileSync(resolve(repoRoot, rollingPath), "utf8");
const classifier = readFileSync(resolve(repoRoot, classifierPath), "utf8");
const lib = readFileSync(resolve(repoRoot, libPath), "utf8");
const documentation = readFileSync(resolve(repoRoot, documentationPath), "utf8");
const profilesCheckpoint = readFileSync(
  resolve(repoRoot, profilesCheckpointPath),
  "utf8",
);
const plan = readFileSync(resolve(repoRoot, planPath), "utf8");
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
  contract.packet !== "dlq-duplicate-moving-window-scan-source" ||
  contract.status !== "source_complete_server_composition_runtime_pending" ||
  contract.owner !== "rustok-iggy" ||
  contract.feature !== "iggy" ||
  contract.source !== sourcePath ||
  contract.rolling_source !== rollingPath ||
  contract.classifier_source !== classifierPath ||
  contract.verifier !== verifierPath ||
  contract.documentation !== documentationPath ||
  contract.profiles_checkpoint !== profilesCheckpointPath ||
  contract.execution_status !== "source_not_run"
) {
  fail("moving-window scan contract identity or source status drift");
}

if (!same(contract.public_exports, expectedExports)) {
  fail("moving-window scan public export allowlist drift");
}
if (!same(contract.required_source_tests, expectedTests)) {
  fail("moving-window scan focused test allowlist drift");
}

if (
  contract.bounds?.maximum_partitions !== 128 ||
  contract.bounds?.maximum_total_messages_per_cycle !== 10000 ||
  contract.bounds?.maximum_batch_messages !== 1000 ||
  contract.bounds?.equal_per_partition_message_budget !== true ||
  contract.bounds?.rolling_cycle_capacity_must_cover_fair_cycle !== true ||
  contract.bounds?.production_defaults !== false
) {
  fail("moving-window scan bounds drift");
}

if (
  contract.cursor_semantics?.one_private_next_offset_per_partition !== true ||
  contract.cursor_semantics?.advance_only_after_complete_all_partition_cycle !== true ||
  contract.cursor_semantics?.empty_partition_cursor_unchanged !== true ||
  contract.cursor_semantics?.failed_cycle_preserves_all_cursors !== true ||
  contract.cursor_semantics?.explicit_offsets !== true ||
  contract.cursor_semantics?.auto_commit !== false ||
  contract.cursor_semantics?.stored_consumer_offsets !== false
) {
  fail("moving-window cursor semantics drift");
}

if (
  contract.rolling_integration?.complete_cycle_only !== true ||
  contract.rolling_integration?.combined_observations_not_exported !== true ||
  contract.rolling_integration?.failed_cycle_preserves_rolling_state !== true ||
  contract.rolling_integration?.cross_cycle_ordinary_duplicate !== true ||
  contract.rolling_integration?.cross_cycle_identity_conflict !== true ||
  contract.rolling_integration?.history_truncated_delegated !== true
) {
  fail("moving-window rolling integration drift");
}

if (
  contract.restart_semantics?.progress_persisted !== false ||
  contract.restart_semantics?.rolling_state_persisted !== false ||
  contract.restart_semantics?.new_process_starts_at_reviewed_initial_offset !== true ||
  contract.restart_semantics?.explicit_reset_to_initial_offset !== true ||
  contract.restart_semantics?.reset_clears_rolling_history !== true ||
  contract.restart_semantics?.reset_generation_count_only !== true ||
  contract.restart_semantics?.restart_safe_progress_claimed !== false
) {
  fail("moving-window restart/reset semantics drift");
}

if (
  !same(contract.snapshot_fields, [
    "rolling",
    "partition_count",
    "advanced_partitions",
    "reset_generation",
  ])
) {
  fail("moving-window snapshot field allowlist drift");
}

for (const [operation, allowed] of Object.entries(contract.mutation_boundary ?? {})) {
  if (allowed !== false) fail(`moving-window mutation became allowed: ${operation}`);
}

if (
  contract.privacy_boundary?.identifier_free_snapshot !== true ||
  contract.privacy_boundary?.payload_free_snapshot !== true ||
  contract.privacy_boundary?.cursor_values_not_exported !== true ||
  contract.privacy_boundary?.observations_not_exported !== true
) {
  fail("moving-window privacy flags drift");
}

const requiredExcluded = new Set([
  "broker_address",
  "stream",
  "topic",
  "partition_id",
  "offset",
  "broker_message_id",
  "payload",
  "payload_sha256",
  "receipt_identity",
  "publisher_identity",
  "credential",
  "timestamp",
  "raw_iggy_error",
]);
for (const field of contract.privacy_boundary?.snapshot_excludes ?? []) {
  requiredExcluded.delete(field);
}
if (requiredExcluded.size > 0) {
  fail(`moving-window privacy exclusions are incomplete: ${[...requiredExcluded].join(", ")}`);
}

if (
  !same(contract.stable_errors, {
    invalid_policy: "iggy.dlq_duplicate.moving_window_policy_invalid",
    poll_failed: "iggy.dlq_duplicate.moving_window_poll_failed",
    invalid_response: "iggy.dlq_duplicate.moving_window_response_invalid",
    offset_overflow: "iggy.dlq_duplicate.moving_window_offset_overflow",
    invalid_cycle: "iggy.dlq_duplicate.moving_window_cycle_invalid",
    count_overflow: "iggy.dlq_duplicate.moving_window_count_overflow",
    reset_overflow: "iggy.dlq_duplicate.moving_window_reset_overflow",
  })
) {
  fail("moving-window stable error code drift");
}

for (const marker of [
  'const READ_ONLY_CONSUMER: &str = "rustok-dlq-duplicate-moving-readonly-v1";',
  "pub struct IggyDlqDuplicateMovingWindowPolicy",
  ".checked_mul(partition_count)",
  ".filter(|total| *total <= MAX_SCAN_MESSAGES)",
  "rolling_policy.max_observations_per_cycle() < total_message_budget",
  "pub struct IggyDlqDuplicateMovingWindowSnapshot",
  "pub const fn progress_persisted(&self) -> bool",
  "pub const fn restart_resets_to_initial_offset(&self) -> bool",
  "pub struct IggyDlqDuplicateMovingWindowState",
  "cursors: BTreeMap<u32, u64>",
  "pub fn reset_to_initial_offset(",
  "fn apply_complete_cycle(",
  "let mut candidate_cursors = self.cursors.clone();",
  "let rolling = self.rolling.push_cycle(observations)?;",
  "self.cursors = candidate_cursors;",
  "pub struct IggyDlqDuplicateMovingWindowScanner<'a>",
  "pub async fn scan_cycle(",
  ".poll_messages(",
  "&PollingStrategy::offset(next_offset)",
  "requested_count,\n                    false,",
  "pub enum IggyDlqDuplicateMovingWindowError",
  '"iggy.dlq_duplicate.moving_window_policy_invalid"',
  '"iggy.dlq_duplicate.moving_window_poll_failed"',
  '"iggy.dlq_duplicate.moving_window_response_invalid"',
  '"iggy.dlq_duplicate.moving_window_offset_overflow"',
  '"iggy.dlq_duplicate.moving_window_cycle_invalid"',
  '"iggy.dlq_duplicate.moving_window_count_overflow"',
  '"iggy.dlq_duplicate.moving_window_reset_overflow"',
]) {
  requireText("moving-window scan source", source, marker);
}

for (const testName of expectedTests) {
  requireText("moving-window scan source tests", source, `fn ${testName}()`);
}
if (countText(source, "#[test]") !== expectedTests.length) {
  fail("moving-window scan source must contain exactly five focused unit tests");
}

for (const marker of [
  "pub cursors:",
  "pub observations:",
  "pub partition_id:",
  "pub start_offset:",
  "pub next_offset:",
  "Serialize",
  "Deserialize",
  "ConsumerKind::ConsumerGroup",
  "PollingStrategy::next(",
  ".store_consumer_offset(",
  "ConsumerOffsetClient",
  ".acknowledge(",
  ".delete_stream(",
  ".delete_topic(",
  ".purge_topic(",
  ".send_messages(",
  ".move_to_dlq(",
  ".retry_entry(",
  ".reserve_and_claim(",
  ".mark_published(",
  ".mark_acknowledged(",
  ".shutdown(",
]) {
  forbidText("moving-window scan source", source, marker);
}

for (const marker of [
  "pub struct DlqDuplicateRollingWindow",
  "pub fn push_cycle(",
  "history_truncated: evicted_cycles > 0",
]) {
  requireText("rolling-window source", rolling, marker);
}
for (const marker of [
  "pub struct DlqDuplicateObservation",
  "pub struct DlqDuplicateSummary",
  "pub fn summarize_dlq_duplicates(",
]) {
  requireText("duplicate classifier source", classifier, marker);
}

requireText(
  "rustok-iggy module list",
  lib,
  '#[cfg(feature = "iggy")]\npub mod dlq_duplicate_moving_window_scan;',
);
for (const exportName of expectedExports) {
  requireText("rustok-iggy public exports", lib, exportName);
}

for (const marker of [
  "independent private partition cursors",
  "Complete-cycle atomicity",
  "Progress persistence is deliberately **not**",
  "reset_to_initial_offset()",
  "server composition and runtime evidence pending",
]) {
  requireText("moving-window owner guide", documentation, marker);
}
for (const marker of [
  "Profiles still never authorizes",
  "private per-partition next offsets",
  "explicit reset",
  "restart-safe progress",
  "server observer mode",
]) {
  requireText("Profiles moving-window checkpoint", profilesCheckpoint, marker);
}
for (const marker of [
  "moving-window scanner integration is source-complete",
  "independent process-local per-partition cursors",
  "restart-reset semantics",
  "Compose the moving duplicate observer",
  "verify-iggy-dlq-duplicate-moving-window-scan.mjs",
]) {
  requireText("canonical Profiles plan", plan, marker);
}

const requiredRemaining = new Set([
  "compose_explicit_opt_in_mode_aware_server_observer",
  "define_reviewed_configuration_surface",
  "retain_external_iggy_cross_cycle_runtime_evidence",
  "define_persisted_cursor_store_only_if_restart_continuity_is_required",
  "define_identifier_free_telemetry_and_health_projection",
]);
for (const item of contract.remaining_work ?? []) requiredRemaining.delete(item);
if (requiredRemaining.size > 0) {
  fail(`moving-window remaining work drift: ${[...requiredRemaining].join(", ")}`);
}

if (failures.length > 0) {
  console.error("Iggy DLQ duplicate moving-window scan verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Iggy DLQ duplicate moving-window scan source verified: bounded equal-budget polling, private independent per-partition cursors, complete-cycle atomic rolling updates, explicit non-persistent restart reset, identifier-free snapshots, and no broker/receipt mutations are locked; server composition and external runtime evidence remain pending.",
);
