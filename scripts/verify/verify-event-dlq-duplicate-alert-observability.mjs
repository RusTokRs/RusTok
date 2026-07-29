#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
const contractPath =
  "crates/rustok-iggy/contracts/evidence/dlq-duplicate-alert-observability-source.json";
const contract = JSON.parse(readFileSync(resolve(root, contractPath), "utf8"));
const telemetry = readFileSync(resolve(root, contract.telemetry_source), "utf8");
const registration = readFileSync(resolve(root, contract.telemetry_registration), "utf8");
const observer = readFileSync(resolve(root, contract.observer_source), "utf8");
const projection = readFileSync(resolve(root, contract.projection_source), "utf8");
const bootstrap = readFileSync(resolve(root, contract.bootstrap_source), "utf8");
const services = readFileSync(resolve(root, contract.service_registry_source), "utf8");
const runtime = readFileSync(resolve(root, contract.runtime_source), "utf8");
const docs = readFileSync(resolve(root, contract.documentation), "utf8");
const checkpoint = readFileSync(resolve(root, contract.profiles_checkpoint), "utf8");
const failures = [];

const fail = (message) => failures.push(message);
const same = (actual, expected) => JSON.stringify(actual) === JSON.stringify(expected);
const requireText = (name, text, marker) => {
  if (!text.includes(marker)) fail(`${name} is missing required marker: ${marker}`);
};

if (
  contract.schema_version !== 1 ||
  contract.module !== "event-delivery" ||
  contract.packet !== "dlq-duplicate-alert-observability-source" ||
  contract.status !== "source_complete_runtime_execution_pending" ||
  contract.execution_status !== "not_run"
) {
  fail("observability source contract identity or status drift");
}

if (
  contract.health_projection?.type !== "EventDlqDuplicateAlertHealthSnapshot" ||
  contract.health_projection?.affects_readiness !== false ||
  !same(contract.health_projection?.states, [
    "disabled",
    "not_applicable",
    "starting",
    "available",
    "unavailable",
    "stopped",
  ])
) {
  fail("identifier-free health projection boundary drift");
}

const expectedMetrics = [
  "rustok_dlq_duplicate_alert_observer_state",
  "rustok_dlq_duplicate_alert_snapshots_total",
  "rustok_dlq_duplicate_alert_evaluation_flags",
];
if (!same(contract.prometheus_metrics?.map((metric) => metric.name), expectedMetrics)) {
  fail("Prometheus metric family identity drift");
}

if (
  !same(contract.bounded_evaluation_flags, [
    "physical_duplicates",
    "identity_conflict",
    "duplicate_messages_threshold",
    "duplicate_groups_threshold",
    "max_copies_threshold",
  ]) ||
  contract.projection_semantics?.failure_stage_inference !== false ||
  contract.projection_semantics?.snapshot_recorded_only_after_generation_change !== true
) {
  fail("bounded label or inference-free projection drift");
}

for (const marker of [
  "const HEALTH_STATE_LABELS: [&str; 6]",
  "const EVALUATION_FLAGS: [&str; 5]",
  "pub static ref DLQ_DUPLICATE_ALERT_OBSERVER_STATE",
  "pub static ref DLQ_DUPLICATE_ALERT_SNAPSHOTS_TOTAL",
  "pub static ref DLQ_DUPLICATE_ALERT_EVALUATION_FLAGS",
  '["deployment", "scan_mode", "state"]',
  '["deployment", "scan_mode", "availability", "level"]',
  '["deployment", "scan_mode", "flag"]',
  "pub fn record_state(",
  "pub fn record_snapshot(",
  "metric_labels_are_closed_and_identifier_free",
]) {
  requireText("telemetry source", telemetry, marker);
}
if (telemetry.includes("failure_stage") || telemetry.includes("record_failure")) {
  fail("telemetry source infers a failure stage it cannot know");
}

for (const marker of [
  "pub mod dlq_duplicate_alert_metrics;",
  "dlq_duplicate_alert_metrics::register(registry)?;",
]) {
  requireText("telemetry registration", registration, marker);
}

for (const marker of [
  "pub enum EventDlqDuplicateAlertHealthState",
  "pub struct EventDlqDuplicateAlertHealthSnapshot",
  "pub struct EventDlqDuplicateAlertObservabilityHandle",
  "pub fn start_event_dlq_duplicate_alert_observability",
  "pub const fn affects_readiness(self) -> bool",
  "fn collect_projection(",
  "fn project_runtime(",
  "fn record_projection(",
  "let generation_changed =",
  "health_projection_tracks_identifier_free_runtime_transitions",
  "static_modes_have_no_scan_or_readiness_effect",
  "scan_mode_labels_are_bounded",
]) {
  requireText("server projection", projection, marker);
}

for (const marker of [
  "pub const fn mode(&self) -> EventDlqDuplicateAlertObserverMode",
  "pub fn current_snapshot(&self)",
  "pub fn is_finished(&self)",
]) {
  requireText("existing observer", observer, marker);
}

requireText(
  "server bootstrap",
  bootstrap,
  "start_event_dlq_duplicate_alert_observability(",
);
requireText(
  "service registry",
  services,
  "pub mod event_dlq_duplicate_alert_observability;",
);

for (const marker of [
  "latest-value snapshot for duplicate alert telemetry and health consumers",
  "affect readiness",
]) {
  requireText("runtime source", runtime, marker);
}

for (const marker of [
  "No readiness coupling",
  "rustok_dlq_duplicate_alert_observer_state",
  "bounded labels",
  "source-complete",
]) {
  requireText("owner documentation", docs, marker);
}

for (const marker of [
  "Profiles authorization",
  "does not authorize",
  "readiness",
  "source-complete",
]) {
  requireText("Profiles checkpoint", checkpoint, marker);
}

for (const excluded of contract.privacy_exclusions ?? []) {
  if (contract.prometheus_metrics.flatMap((metric) => metric.labels).includes(excluded)) {
    fail(`privacy-excluded label leaked into metrics: ${excluded}`);
  }
}

if (
  contract.mutation_boundary?.readiness_coupling !== false ||
  contract.mutation_boundary?.liveness_coupling !== false ||
  contract.mutation_boundary?.profiles_authorization !== false
) {
  fail("observability mutation/readiness boundary drift");
}

if (failures.length > 0) {
  console.error("DLQ duplicate alert observability source verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "DLQ duplicate alert observability source verified: bounded Prometheus labels, generation-aware companion projection, privacy exclusions, inference-free failure handling, and no readiness coupling are locked; runtime execution remains pending.",
);
