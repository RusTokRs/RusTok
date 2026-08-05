#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-server-observer-source.json";
const movingContractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-moving-window-scan-source.json";
const iggySourcePath = "crates/rustok-iggy/src/dlq_duplicate_alert_observer.rs";
const movingSourcePath =
  "crates/rustok-iggy/src/dlq_duplicate_moving_window_scan.rs";
const serverSourcePath =
  "apps/server/src/services/event_dlq_duplicate_alert_observer.rs";
const bootstrapPath = "apps/server/src/services/server_bootstrap.rs";
const servicesPath = "apps/server/src/services/mod.rs";
const libPath = "crates/rustok-iggy/src/lib.rs";
const runtimeSourcePath = "crates/rustok-iggy/src/dlq_duplicate_alert_runtime.rs";
const scannerSourcePath = "crates/rustok-iggy/src/dlq_duplicate_external_scan.rs";
const documentationPath =
  "crates/rustok-iggy/docs/dlq-duplicate-alert-server-observer.md";
const profilesCheckpointPath =
  "crates/rustok-profiles/docs/poison-duplicate-alert-server-observer-checkpoint.md";
const planPath = "crates/rustok-profiles/docs/implementation-plan.md";
const verifierPath =
  "scripts/verify/verify-event-dlq-duplicate-alert-server-observer.mjs";

const contract = JSON.parse(readFileSync(resolve(repoRoot, contractPath), "utf8"));
const movingContract = JSON.parse(
  readFileSync(resolve(repoRoot, movingContractPath), "utf8"),
);
const iggySource = readFileSync(resolve(repoRoot, iggySourcePath), "utf8");
const movingSource = readFileSync(resolve(repoRoot, movingSourcePath), "utf8");
const serverSource = readFileSync(resolve(repoRoot, serverSourcePath), "utf8");
const bootstrap = readFileSync(resolve(repoRoot, bootstrapPath), "utf8");
const services = readFileSync(resolve(repoRoot, servicesPath), "utf8");
const lib = readFileSync(resolve(repoRoot, libPath), "utf8");
const runtimeSource = readFileSync(resolve(repoRoot, runtimeSourcePath), "utf8");
const scannerSource = readFileSync(resolve(repoRoot, scannerSourcePath), "utf8");
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
  contract.schema_version !== 3 ||
  contract.module !== "event-delivery" ||
  contract.packet !== "dlq-duplicate-alert-server-observer-source" ||
  contract.status !== "source_complete_runtime_execution_pending" ||
  !same(contract.owners, ["rustok-server", "rustok-iggy"]) ||
  contract.iggy_source !== iggySourcePath ||
  contract.moving_scan_source !== movingSourcePath ||
  contract.moving_scan_contract !== movingContractPath ||
  contract.server_source !== serverSourcePath ||
  contract.bootstrap_source !== bootstrapPath ||
  contract.service_registry_source !== servicesPath ||
  contract.verifier !== verifierPath ||
  contract.documentation !== documentationPath ||
  contract.profiles_checkpoint !== profilesCheckpointPath ||
  contract.execution_status !== "source_not_run"
) {
  fail("DLQ duplicate alert server observer identity or status drift");
}

if (
  movingContract.packet !== "dlq-duplicate-moving-window-scan-source" ||
  movingContract.status !== "source_complete_server_composed_runtime_pending" ||
  movingContract.source !== movingSourcePath ||
  movingContract.server_composition?.status !== "source_complete_runtime_pending"
) {
  fail("moving-window source contract relationship drift");
}

if (
  !same(contract.delivery_profiles, {
    outbox_local: "not_applicable",
    outbox_iggy: "iggy_observer",
  }) ||
  !same(contract.iggy_modes, {
    bundled: "connect_to_existing_loopback_broker",
    external: "connect_to_reviewed_external_addresses",
  })
) {
  fail("event-delivery or Iggy observer matrix drift");
}

if (
  contract.activation?.default_enabled !== false ||
  contract.activation?.enable_env !== "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_ENABLED" ||
  contract.activation?.background_workers_required !== true ||
  contract.activation?.event_runtime_required !== true ||
  contract.activation?.outbox_local_requires_iggy !== false ||
  contract.activation?.outbox_iggy_requires_shared_transport !== true ||
  contract.activation?.outbox_iggy_missing_active_mode_fails_closed !== true ||
  contract.activation?.observer_startup_failure_is_non_fatal !== true ||
  contract.activation?.startup_failure_mode !== "unavailable" ||
  contract.activation?.non_iggy_profiles_are_errors !== false
) {
  fail("DLQ duplicate observer activation boundary drift");
}

const scan = contract.scan ?? {};
const globalMode = scan.global_budget_mode ?? {};
const fairMode = scan.fair_window_mode ?? {};
const movingMode = scan.moving_window_mode ?? {};
if (
  scan.topic !== "dlq" ||
  scan.configured_domain_partition_allowlist !== true ||
  scan.scan_mode_env !== "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE" ||
  scan.default_scan_mode !== "global_budget" ||
  !same(globalMode.accepted_values, ["global", "global_budget"]) ||
  globalMode.start_offset_env !== "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_START_OFFSET" ||
  globalMode.start_offset_default !== 0 ||
  globalMode.max_messages_env !== "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_MAX_MESSAGES" ||
  globalMode.default_max_messages !== 1000 ||
  globalMode.one_global_message_budget !== true ||
  globalMode.later_partition_starvation_possible !== true ||
  globalMode.cross_cycle_accumulation !== false ||
  !same(fairMode.accepted_values, ["fair", "fair_window"]) ||
  fairMode.start_offset_env !== "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_START_OFFSET" ||
  fairMode.start_offset_default !== 0 ||
  fairMode.per_partition_messages_env !==
    "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_PER_PARTITION_MESSAGES" ||
  fairMode.per_partition_messages_required !== true ||
  fairMode.equal_per_partition_message_budget !== true ||
  fairMode.every_partition_attempted_on_successful_scan !== true ||
  fairMode.total_message_budget_checked_by_scanner !== true ||
  fairMode.maximum_total_messages !== 10000 ||
  fairMode.all_partition_observations_combined_before_classification !== true ||
  fairMode.cross_cycle_accumulation !== false ||
  !same(movingMode.accepted_values, ["moving", "moving_window"]) ||
  movingMode.explicit_opt_in !== true ||
  movingMode.initial_offset_env !== "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_START_OFFSET" ||
  movingMode.initial_offset_required !== true ||
  movingMode.per_partition_messages_env !==
    "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_PER_PARTITION_MESSAGES" ||
  movingMode.per_partition_messages_required !== true ||
  movingMode.batch_size_env !== "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_BATCH_SIZE" ||
  movingMode.batch_size_required !== true ||
  movingMode.rolling_max_cycles_env !==
    "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_ROLLING_MAX_CYCLES" ||
  movingMode.rolling_max_cycles_required !== true ||
  movingMode.rolling_max_observations_per_cycle_env !==
    "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_ROLLING_MAX_OBSERVATIONS_PER_CYCLE" ||
  movingMode.rolling_max_observations_per_cycle_required !== true ||
  movingMode.production_defaults !== false ||
  movingMode.one_private_next_offset_per_partition !== true ||
  movingMode.complete_all_partition_cycle_before_mutation !== true ||
  movingMode.rolling_cycle_capacity_must_cover_fair_cycle !== true ||
  movingMode.cross_cycle_duplicate_accumulation !== true ||
  movingMode.failed_cycle_preserves_process_local_state !== true ||
  movingMode.progress_persisted !== false ||
  movingMode.new_connection_or_process_starts_at_reviewed_initial_offset !== true ||
  movingMode.current_tail_coverage_claimed !== false ||
  movingMode.complete_history_claimed !== false ||
  scan.batch_size_env !== "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_BATCH_SIZE" ||
  scan.fixed_mode_default_batch_size !== 100 ||
  scan.auto_commit !== false ||
  scan.stored_offset_polling !== false
) {
  fail("DLQ duplicate observer scan-mode boundary drift");
}

if (
  contract.runtime?.publisher !== "DlqDuplicateAlertRuntimePublisher" ||
  contract.runtime?.initial_snapshot_unavailable !== true ||
  contract.runtime?.success_publishes_identifier_free_evaluation !== true ||
  contract.runtime?.moving_snapshot_reduced_to_dlq_duplicate_summary !== true ||
  contract.runtime?.startup_failure_records_unavailable_without_task !== true ||
  contract.runtime?.connection_failure_marks_unavailable !== true ||
  contract.runtime?.scan_failure_marks_unavailable !== true ||
  contract.runtime?.fixed_scan_failure_reconnects !== true ||
  contract.runtime?.moving_scan_failure_retries_same_state !== true ||
  contract.runtime?.shutdown_marks_unavailable !== true ||
  contract.runtime?.event_delivery_remains_active !== true
) {
  fail("DLQ duplicate observer runtime boundary drift");
}

if (
  !same(contract.startup_stable_codes, {
    configuration_invalid:
      "iggy.dlq_duplicate.alert_server_observer_configuration_invalid",
    runtime_unavailable:
      "iggy.dlq_duplicate.alert_server_observer_runtime_unavailable",
  })
) {
  fail("DLQ duplicate observer startup stable-code drift");
}

for (const [operation, allowed] of Object.entries(contract.lifecycle_boundary ?? {})) {
  if (allowed !== false) fail(`observer lifecycle coupling became allowed: ${operation}`);
}
for (const [operation, allowed] of Object.entries(contract.mutation_boundary ?? {})) {
  if (allowed !== false) fail(`observer mutation became allowed: ${operation}`);
}

if (
  contract.policy?.all_six_thresholds_required_when_active !== true ||
  contract.policy?.production_threshold_defaults !== false ||
  contract.policy?.notification_routing !== false ||
  contract.policy?.cooldown_or_suppression !== false
) {
  fail("observer policy boundary drift");
}

const expectedIggyTests = [
  "bundled_mode_requires_matching_loopback_address",
  "all_configured_partitions_are_included_in_request",
  "global_fair_and_moving_scan_modes_remain_explicit",
  "moving_window_requires_complete_cycle_capacity",
  "invalid_partition_count_fails_closed",
  "stable_errors_expose_no_connection_details",
];
const expectedServerTests = [
  "every_event_delivery_profile_has_an_explicit_observer_mode",
  "startup_unavailable_state_has_no_task_or_snapshot",
  "scan_mode_parser_preserves_global_default_and_explicit_opt_in_modes",
  "moving_window_configuration_is_explicit_and_fail_closed",
  "boolean_parser_is_bounded",
];
if (!same(contract.required_iggy_tests, expectedIggyTests)) {
  fail("Iggy observer source test allowlist drift");
}
if (!same(contract.required_server_tests, expectedServerTests)) {
  fail("server observer source test allowlist drift");
}
for (const testName of expectedIggyTests) {
  requireText("Iggy observer tests", iggySource, `fn ${testName}()`);
}
for (const testName of expectedServerTests) {
  requireText("server observer tests", serverSource, `fn ${testName}()`);
}
if (countText(iggySource, "#[test]") !== expectedIggyTests.length) {
  fail("Iggy observer source must contain exactly six focused tests");
}
if (countText(serverSource, "#[test]") !== expectedServerTests.length) {
  fail("server observer source must contain exactly five focused tests");
}

for (const marker of [
  "pub enum IggyDlqDuplicateAlertScanMode",
  "GlobalBudget",
  "FairWindow",
  "MovingWindow",
  "pub struct IggyDlqDuplicateAlertMovingWindowConfig",
  "DlqDuplicateRollingWindowPolicy::new(",
  "IggyDlqDuplicateMovingWindowPolicy::new(",
  "pub struct IggyDlqDuplicateAlertObserver",
  "scan: IggyDlqDuplicateAlertScan",
  "pub async fn connect(",
  "pub async fn connect_fair_window(",
  "pub async fn connect_moving_window(",
  "IggyDlqDuplicateMovingWindowState::new(",
  "pub const fn preserves_process_local_state_after_scan_error",
  "pub async fn summarize(",
  "IggyDlqDuplicateMovingWindowScanner::new(client, stream_name)?",
  "scanner.scan_cycle(state).await?",
  "Ok(*snapshot.rolling().summary())",
  "config.mode == IggyMode::Bundled",
  "is_bundled_loopback_address(",
  "config.topology.domain_partitions > 128",
  '"iggy.dlq_duplicate.alert_observer_configuration_invalid"',
  '"iggy.dlq_duplicate.alert_observer_connection_unavailable"',
]) {
  requireText("Iggy DLQ duplicate alert observer", iggySource, marker);
}

for (const marker of [
  'const ENABLE_ENV: &str = "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_ENABLED";',
  'const SCAN_MODE_ENV: &str = "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_SCAN_MODE";',
  '"RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_PER_PARTITION_MESSAGES"',
  '"RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_ROLLING_MAX_CYCLES"',
  '"RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_ROLLING_MAX_OBSERVATIONS_PER_CYCLE"',
  '"iggy.dlq_duplicate.alert_server_observer_configuration_invalid"',
  '"iggy.dlq_duplicate.alert_server_observer_runtime_unavailable"',
  "pub enum EventDlqDuplicateAlertObserverMode",
  "NotApplicableOutboxLocal",
  "IggyBundled",
  "IggyExternal",
  "EventDlqDuplicateAlertScanConfig::GlobalBudget",
  "EventDlqDuplicateAlertScanConfig::FairWindow",
  "EventDlqDuplicateAlertScanConfig::MovingWindow",
  "IggyDlqDuplicateAlertObserver::connect(",
  "IggyDlqDuplicateAlertObserver::connect_fair_window(",
  "IggyDlqDuplicateAlertObserver::connect_moving_window(",
  '"global" | "global_budget"',
  '"fair" | "fair_window"',
  '"moving" | "moving_window"',
  "required_u64_env(START_OFFSET_ENV)",
  "required_u32_env(ROLLING_MAX_CYCLES_ENV)",
  "required_u32_env(ROLLING_MAX_OBSERVATIONS_PER_CYCLE_ENV)",
  "connected.summarize().await",
  "connected.preserves_process_local_state_after_scan_error()",
  "preserves_process_local_state = preserve_state",
  "publisher.publish(&summary)",
  "publisher.mark_unavailable()",
  "event delivery remains active",
  "retry remains isolated from event delivery",
]) {
  requireText("mode-aware server observer", serverSource, marker);
}

const nonApplicableIndex = serverSource.indexOf(
  "EventDlqDuplicateAlertObserverMode::NotApplicableOutboxLocal",
);
const sharedTransportIndex = serverSource.indexOf("ctx.shared_get::<Arc<IggyTransport>>()");
if (
  nonApplicableIndex < 0 ||
  sharedTransportIndex < 0 ||
  nonApplicableIndex > sharedTransportIndex
) {
  fail("non-Iggy delivery profiles must exit before shared Iggy transport access");
}

for (const marker of [
  "IggyTransport::new(",
  "BundledConnector::new(",
  ".shutdown(",
  ".store_offset(",
  ".store_consumer_offset(",
  ".acknowledge(",
  ".send_messages(",
  ".delete_stream(",
  ".delete_topic(",
  ".purge_topic(",
  ".reserve_and_claim(",
  ".mark_published(",
  ".mark_acknowledged(",
  ".notify(",
  ".page(",
]) {
  forbidText("observer sources", `${iggySource}\n${serverSource}`, marker);
}

for (const marker of [
  "Serialize",
  "Deserialize",
  "broker_address",
  "payload_sha256",
  "raw_client_error",
  "pub initial_offset:",
  "pub cursors:",
]) {
  forbidText("runtime observer structs", `${iggySource}\n${serverSource}`, marker);
}

requireText(
  "runtime composition",
  runtimeSource,
  "pub struct DlqDuplicateAlertRuntimePublisher",
);
requireText("runtime composition", runtimeSource, "pub fn mark_unavailable(");
requireText(
  "bounded fixed scanner",
  scannerSource,
  "pub struct IggyDlqDuplicateScanWindowPolicy",
);
requireText(
  "moving scanner",
  movingSource,
  "pub struct IggyDlqDuplicateMovingWindowState",
);
requireText(
  "moving scanner",
  movingSource,
  "pub async fn scan_cycle(",
);
requireText(
  "Iggy module registry",
  lib,
  "pub mod dlq_duplicate_alert_observer;",
);
for (const exportName of contract.public_iggy_exports ?? []) {
  requireText("Iggy public exports", lib, exportName);
}
requireText(
  "server service registry",
  services,
  "pub mod event_dlq_duplicate_alert_observer;",
);
requireText(
  "server bootstrap",
  bootstrap,
  "start_event_dlq_duplicate_alert_observer(",
);

const requiredPrivacyExclusions = new Set([
  "broker_address",
  "stream",
  "topic",
  "partition",
  "offset",
  "broker_message_id",
  "payload",
  "payload_sha256",
  "receipt_identity",
  "credential",
  "raw_client_error",
  "raw_threshold_values",
  "source_counts",
  "private_cursor_values",
  "rolling_observations",
]);
for (const field of contract.privacy_boundary?.logs_and_snapshots_exclude ?? []) {
  requiredPrivacyExclusions.delete(field);
}
if (requiredPrivacyExclusions.size > 0) {
  fail(
    `observer privacy exclusions are incomplete: ${[
      ...requiredPrivacyExclusions,
    ].join(", ")}`,
  );
}
if (
  contract.privacy_boundary?.moving_configuration_debug_excludes_initial_offset !== true ||
  contract.privacy_boundary?.stable_error_codes_only !== true ||
  contract.privacy_boundary?.serialization_added !== false ||
  contract.privacy_boundary?.persistence_added !== false
) {
  fail("observer privacy flags drift");
}

for (const marker of [
  "moving_window",
  "ROLLING_MAX_CYCLES",
  "process-local",
  "complete cycle",
  "global_budget remains the default",
  "runtime execution pending",
]) {
  requireText("observer documentation", documentation, marker);
}
for (const marker of [
  "moving_window",
  "Profiles authorization",
  "private cursor",
  "restart",
  "source complete",
]) {
  requireText("Profiles observer checkpoint", profilesCheckpoint, marker);
}
for (const marker of [
  "moving-window server observer composition is source-complete",
  "reviewed fail-closed configuration",
  "external-Iggy cross-cycle",
  "Execute moving duplicate observer evidence",
]) {
  requireText("canonical Profiles plan", plan, marker);
}

const requiredRemaining = new Set([
  "fair_window_external_iggy_execution_evidence",
  "moving_window_external_iggy_cross_cycle_execution_evidence",
  "review_reset_frequency_and_initial_offset_per_deployment",
  "persisted_cursor_owner_only_if_restart_continuity_is_required",
  "telemetry_projection_outside_observer",
  "health_projection_without_readiness_coupling",
  "notification_delivery_and_suppression",
  "retained_server_observer_execution_evidence",
  "authorized_destructive_reconciliation",
]);
for (const item of contract.remaining_work ?? []) requiredRemaining.delete(item);
if (requiredRemaining.size > 0) {
  fail(`observer remaining work drift: ${[...requiredRemaining].join(", ")}`);
}

if (failures.length > 0) {
  console.error("Event DLQ duplicate alert server observer verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Event DLQ duplicate alert server observer source verified: default-off activation, explicit OutboxLocal not-applicable handling, non-fatal Unavailable startup, compatibility global and fixed fair scans, explicit moving-window configuration with independent process-local cursors and atomic rolling cycles, identifier-free summary publication, moving-state preservation after failed cycles, and no event-delivery/readiness/Profile mutation are locked; external runtime execution remains pending.",
);
