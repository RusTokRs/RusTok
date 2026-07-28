#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-server-observer-source.json";
const iggySourcePath = "crates/rustok-iggy/src/dlq_duplicate_alert_observer.rs";
const serverSourcePath =
  "apps/server/src/services/event_dlq_duplicate_alert_observer.rs";
const bootstrapPath = "apps/server/src/services/server_bootstrap.rs";
const servicesPath = "apps/server/src/services/mod.rs";
const libPath = "crates/rustok-iggy/src/lib.rs";
const runtimeSourcePath = "crates/rustok-iggy/src/dlq_duplicate_alert_runtime.rs";
const scannerSourcePath = "crates/rustok-iggy/src/dlq_duplicate_external_scan.rs";

const contract = JSON.parse(readFileSync(resolve(repoRoot, contractPath), "utf8"));
const iggySource = readFileSync(resolve(repoRoot, iggySourcePath), "utf8");
const serverSource = readFileSync(resolve(repoRoot, serverSourcePath), "utf8");
const bootstrap = readFileSync(resolve(repoRoot, bootstrapPath), "utf8");
const services = readFileSync(resolve(repoRoot, servicesPath), "utf8");
const lib = readFileSync(resolve(repoRoot, libPath), "utf8");
const runtimeSource = readFileSync(resolve(repoRoot, runtimeSourcePath), "utf8");
const scannerSource = readFileSync(resolve(repoRoot, scannerSourcePath), "utf8");
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
  contract.module !== "event-delivery" ||
  contract.packet !== "dlq-duplicate-alert-server-observer-source" ||
  contract.status !== "source_complete_runtime_execution_pending" ||
  !sameValue(contract.owners, ["rustok-server", "rustok-iggy"]) ||
  contract.iggy_source !== iggySourcePath ||
  contract.server_source !== serverSourcePath ||
  contract.bootstrap_source !== bootstrapPath ||
  contract.service_registry_source !== servicesPath ||
  contract.execution_status !== "source_not_run"
) {
  fail("DLQ duplicate alert server observer identity or status drift");
}

if (
  !sameValue(contract.delivery_profiles, {
    memory: "not_applicable",
    outbox_local: "not_applicable",
    outbox_iggy: "iggy_observer",
  })
) {
  fail("event delivery profile observer matrix drift");
}
if (
  !sameValue(contract.iggy_modes, {
    bundled: "connect_to_existing_loopback_broker",
    external: "connect_to_reviewed_external_addresses",
  })
) {
  fail("Iggy bundled/external observer matrix drift");
}
if (
  contract.activation?.default_enabled !== false ||
  contract.activation?.enable_env !== "RUSTOK_EVENT_DLQ_DUPLICATE_ALERT_ENABLED" ||
  contract.activation?.background_workers_required !== true ||
  contract.activation?.event_runtime_required !== true ||
  contract.activation?.memory_requires_iggy !== false ||
  contract.activation?.outbox_local_requires_iggy !== false ||
  contract.activation?.outbox_iggy_requires_shared_transport !== true ||
  contract.activation?.outbox_iggy_missing_active_mode_fails_closed !== true ||
  contract.activation?.non_iggy_profiles_are_errors !== false
) {
  fail("DLQ duplicate observer activation boundary drift");
}
if (
  contract.scan?.topic !== "dlq" ||
  contract.scan?.all_configured_domain_partitions !== true ||
  contract.scan?.explicit_start_offset !== true ||
  contract.scan?.bounded_max_messages !== true ||
  contract.scan?.bounded_batch_size !== true ||
  contract.scan?.auto_commit !== false ||
  contract.scan?.stored_offset_polling !== false
) {
  fail("DLQ duplicate observer scan boundary drift");
}
if (
  contract.runtime?.publisher !== "DlqDuplicateAlertRuntimePublisher" ||
  contract.runtime?.initial_snapshot_unavailable !== true ||
  contract.runtime?.success_publishes_identifier_free_evaluation !== true ||
  contract.runtime?.connection_failure_marks_unavailable !== true ||
  contract.runtime?.scan_failure_marks_unavailable !== true ||
  contract.runtime?.shutdown_marks_unavailable !== true ||
  contract.runtime?.reconnect_after_failure !== true ||
  contract.runtime?.event_delivery_remains_active !== true
) {
  fail("DLQ duplicate observer runtime boundary drift");
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
  "all_configured_partitions_are_scanned_once",
  "invalid_partition_count_fails_closed",
  "stable_errors_expose_no_connection_details",
];
const expectedServerTests = [
  "every_event_delivery_profile_has_an_explicit_observer_mode",
  "boolean_parser_is_bounded",
];
if (!sameValue(contract.required_iggy_tests, expectedIggyTests)) {
  fail("Iggy observer source test allowlist drift");
}
if (!sameValue(contract.required_server_tests, expectedServerTests)) {
  fail("server observer source test allowlist drift");
}
for (const testName of expectedIggyTests) {
  requireText("Iggy observer tests", iggySource, `fn ${testName}()`);
}
for (const testName of expectedServerTests) {
  requireText("server observer tests", serverSource, `fn ${testName}()`);
}
if (countText(iggySource, "#[test]") !== expectedIggyTests.length) {
  fail("Iggy observer source must contain exactly four focused tests");
}
if (countText(serverSource, "#[test]") !== expectedServerTests.length) {
  fail("server observer source must contain exactly two focused tests");
}

for (const marker of [
  "pub struct IggyDlqDuplicateAlertObserver",
  "client: IggyClient",
  "request: IggyDlqDuplicateScanRequest",
  "pub async fn connect(",
  "let partitions = configured_partitions(config)?;",
  "IggyDlqDuplicateScanRequest::new(",
  "let external = read_only_connection_config(config)?;",
  "for connection_string in connection_strings",
  "client.connect().await.is_ok()",
  "IggyDlqDuplicateScanner::new(&self.client, &self.stream_name)?",
  ".summarize(&self.request)",
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
  "pub enum EventDlqDuplicateAlertObserverMode",
  "NotApplicableMemory",
  "NotApplicableOutboxLocal",
  "IggyBundled",
  "IggyExternal",
  "pub async fn start_event_dlq_duplicate_alert_observer(",
  "let enabled = optional_bool_env(ENABLE_ENV, false)?;",
  "let mode = observer_mode(runtime.delivery_profile, runtime.iggy_mode.as_ref())?;",
  "EventDeliveryProfile::Memory",
  "EventDeliveryProfile::OutboxLocal",
  "EventDeliveryProfile::OutboxIggy",
  '"outbox_iggy runtime is missing its active Iggy mode"',
  "observer_mode(EventDeliveryProfile::OutboxIggy, None).is_err()",
  "ctx.shared_get::<Arc<IggyTransport>>()",
  "DlqDuplicateAlertRuntimePublisher::new(config.policy)",
  "IggyDlqDuplicateAlertObserver::connect(",
  "connected.summarize().await",
  "publisher.publish(&summary)",
  "publisher.mark_unavailable()",
  "event delivery remains active",
  "reconnecting without affecting event delivery",
]) {
  requireText("mode-aware server observer", serverSource, marker);
}

const nonApplicableIndex = serverSource.indexOf(
  "EventDlqDuplicateAlertObserverMode::NotApplicableMemory",
);
const sharedTransportIndex = serverSource.indexOf("ctx.shared_get::<Arc<IggyTransport>>()");
if (nonApplicableIndex < 0 || sharedTransportIndex < 0 || nonApplicableIndex > sharedTransportIndex) {
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
]) {
  forbidText("runtime observer structs", `${iggySource}\n${serverSource}`, marker);
}

requireText("runtime composition", runtimeSource, "pub struct DlqDuplicateAlertRuntimePublisher");
requireText("runtime composition", runtimeSource, "pub fn mark_unavailable(");
requireText("bounded scanner", scannerSource, "requested_count,\n                        false,");
requireText("Iggy module registry", lib, "pub mod dlq_duplicate_alert_observer;");
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
]);
for (const field of contract.privacy_boundary?.logs_and_snapshots_exclude ?? []) {
  requiredPrivacyExclusions.delete(field);
}
if (requiredPrivacyExclusions.size > 0) {
  fail(`observer privacy exclusions are incomplete: ${[
    ...requiredPrivacyExclusions,
  ].join(", ")}`);
}
if (
  contract.privacy_boundary?.stable_error_codes_only !== true ||
  contract.privacy_boundary?.serialization_added !== false ||
  contract.privacy_boundary?.persistence_added !== false
) {
  fail("observer privacy flags drift");
}

if (
  contract.verifier !==
    "scripts/verify/verify-event-dlq-duplicate-alert-server-observer.mjs" ||
  contract.documentation !==
    "crates/rustok-iggy/docs/dlq-duplicate-alert-server-observer.md" ||
  contract.profiles_checkpoint !==
    "crates/rustok-profiles/docs/poison-duplicate-alert-server-observer-checkpoint.md"
) {
  fail("observer verifier or documentation path drift");
}

if (failures.length > 0) {
  console.error("Event DLQ duplicate alert server observer verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "Event DLQ duplicate alert server observer source verified: default-off activation, explicit Memory/OutboxLocal not-applicable handling, fail-closed OutboxIggy active-mode selection, bundled/external observation, bounded auto_commit=false scans, identifier-free latest-value publication, unavailable/reconnect lifecycle, and no event-delivery/readiness/Profile mutation are locked; runtime execution remains pending.",
);
