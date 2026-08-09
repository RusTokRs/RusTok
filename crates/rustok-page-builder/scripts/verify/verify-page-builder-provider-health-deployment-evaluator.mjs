#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  health: "crates/rustok-page-builder/src/health.rs",
  telemetry: "crates/rustok-telemetry/src/page_builder_provider_metrics.rs",
  identityContract: "crates/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-identity-source.json",
  evaluatorContract: "crates/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-evaluator-source.json",
  evaluator: "scripts/evidence/evaluate-page-builder-provider-health-deployment.mjs",
  overlay: "docs/modules/page-builder-provider-health-deployment-evaluator-actualization-2026-08-09.md",
  parity: "docs/modules/pages-page-builder-plan-parity-actualization-2026-08-08.md",
  pagesGraphql: "crates/rustok-pages/src/graphql/builder_rollout.rs",
  pagesFacade: "crates/rustok-pages/admin/src/builder.rs",
};

const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};

for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(absolute(relativePath))) {
    failures.push(`${label}: missing ${relativePath}`);
    continue;
  }
  const stats = fs.lstatSync(absolute(relativePath));
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${label}: ${relativePath} must be a regular non-symlink file`);
  }
}
if (failures.length > 0) {
  console.error("[verify-page-builder-provider-health-deployment-evaluator] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const contract = JSON.parse(sources.evaluatorContract);

if (contract.format !== "page_builder_provider_health_deployment_evaluator_source_v1") {
  failures.push("evaluator contract format drifted");
}
if (contract.status !== "source_ready_execution_pending") {
  failures.push("evaluator contract must remain source_ready_execution_pending");
}
if (!Array.isArray(contract.predecessors) || !contract.predecessors.includes("page_builder_provider_health_deployment_identity_source_v1")) {
  failures.push("deployment identity source must remain an evaluator predecessor");
}

for (const [key, expected] of Object.entries({
  format: "page_builder_provider_health_deployment_identity_v1",
  required_status: "deployment_identity_verified_health_evaluation_pending",
  source_commit_must_equal_checkout_head: true,
  inventory_complete_must_be_true: true,
  expected_target_count_must_equal_verified_target_count: true,
})) {
  if (contract.identity_input?.[key] !== expected) {
    failures.push(`identity_input.${key} must equal ${JSON.stringify(expected)}`);
  }
}

for (const [key, expected] of Object.entries({
  authority: "maintainer_supplied_complete_prometheus_target_mapping",
  deployment_id_must_equal_identity_packet: true,
  inventory_complete_must_be_true: true,
  target_id_set_must_equal_identity_packet: true,
  target_label_values_unique: true,
  regex_matchers_allowed: false,
  raw_matcher_values_retained: false,
})) {
  if (contract.backend_target_map?.[key] !== expected) {
    failures.push(`backend_target_map.${key} must equal ${JSON.stringify(expected)}`);
  }
}
for (const reserved of ["__name__", "source_commit", "operation", "outcome", "le"]) {
  if (!(contract.backend_target_map?.reserved_matcher_labels ?? []).includes(reserved)) {
    failures.push(`backend target map must reserve ${reserved}`);
  }
}

for (const [key, expected] of Object.entries({
  prometheus_api: "/api/v1/query",
  request_method: "POST",
  redirects_followed: false,
  maximum_parallel_queries: 8,
  query_window_seconds_minimum: 300,
  query_window_seconds_maximum: 86400,
  freshness_seconds_minimum: 60,
  freshness_must_not_exceed_query_window: true,
  identity_capture_must_predate_entire_query_window: true,
  identity_capture_maximum_age_seconds: 86400,
  backend_clock_source: "prometheus_time_function",
})) {
  if (contract.backend_query?.[key] !== expected) {
    failures.push(`backend_query.${key} must equal ${JSON.stringify(expected)}`);
  }
}

for (const [key, expected] of Object.entries({
  current_build_info_required_per_target: true,
  current_build_info_source_commit_must_equal_identity: true,
  current_backend_series_identity_must_be_unique_per_target: true,
  expected_source_build_info_must_exist_in_entire_query_window: true,
  unexpected_source_build_info_in_query_window: "fail_closed",
  partial_target_query_success: "fail_closed",
})) {
  if (contract.source_admission?.[key] !== expected) {
    failures.push(`source_admission.${key} must equal ${JSON.stringify(expected)}`);
  }
}

for (const [key, expected] of Object.entries({
  preview_required_per_target: true,
  publish_required_per_target: true,
  missing_operation_freshness: "fail_closed",
  stale_operation_freshness: "fail_closed",
  future_operation_timestamp_tolerance_seconds: 5,
})) {
  if (contract.freshness_admission?.[key] !== expected) {
    failures.push(`freshness_admission.${key} must equal ${JSON.stringify(expected)}`);
  }
}

for (const [key, expected] of Object.entries({
  duration_series: "rustok_page_builder_provider_operation_duration_seconds_bucket",
  completion_series: "rustok_page_builder_provider_operation_completed_total",
  freshness_series: "rustok_page_builder_provider_last_observation_unix_seconds",
  build_info_series: "rustok_page_builder_provider_build_info",
  counter_reset_aware_function: "increase",
  histogram_quantile: 0.95,
  minimum_preview_samples: 20,
  minimum_publish_samples: 20,
  sample_floor_failure: "fail_closed",
  unknown_operation_or_outcome: "fail_closed",
  non_finite_or_negative_backend_value: "fail_closed",
})) {
  if (contract.aggregation?.[key] !== expected) {
    failures.push(`aggregation.${key} must equal ${JSON.stringify(expected)}`);
  }
}

for (const [key, expected] of Object.entries({
  preview_p95_ms_max: 1500,
  publish_p95_ms_max: 3000,
  sanitize_failure_rate_max: 0.01,
  runtime_error_rate_max: 0.01,
  unavailable_runtime_error_multiplier: 2.0,
  must_match_rust_provider_health_snapshot_evaluate: true,
})) {
  if (contract.provider_health_policy?.[key] !== expected) {
    failures.push(`provider_health_policy.${key} must equal ${JSON.stringify(expected)}`);
  }
}

for (const [key, expected] of Object.entries({
  runtime_evaluation_executed: false,
  deployment_identity_capture_executed_by_this_slice: false,
  pages_provider_health_observed: false,
  pages_reference_consumer_gate_accepted: false,
  forum_wave_accepted: false,
  ffa_promoted: false,
  fba_promoted: false,
  tests_run: false,
  static_verifier_run: false,
  cargo_run: false,
  formatting_run: false,
  workflow_or_ci_run: false,
})) {
  if (contract.non_claims?.[key] !== expected) {
    failures.push(`non_claims.${key} must equal ${JSON.stringify(expected)}`);
  }
}

for (const marker of [
  "pub const PROVIDER_HEALTH_MINIMUM_SAMPLES_PER_OPERATION: usize = 20;",
  "preview_p95_ms: 1500",
  "publish_p95_ms: 3000",
  "sanitize_failure_rate_max: 0.01",
  "runtime_error_rate_max: 0.01",
  "ProviderHealthState::Unavailable",
  "thresholds.runtime_error_rate_max * 2.0",
]) need(sources.health, marker, "Rust provider health policy");

for (const marker of [
  "rustok_page_builder_provider_build_info",
  "rustok_page_builder_provider_operation_duration_seconds",
  "rustok_page_builder_provider_operation_completed_total",
  "rustok_page_builder_provider_last_observation_unix_seconds",
]) need(sources.telemetry, marker, "platform provider metrics");

for (const marker of [
  '"deployment_health_backend_evaluator": "open"',
  '"deployment_identity_capture": "maintainer_execution_pending"',
]) need(sources.identityContract, marker, "identity predecessor cursor");

for (const marker of [
  "--identity",
  "--backend-map",
  "--prometheus-url",
  "--window-seconds",
  "--freshness-seconds",
  "RESERVED_MATCHER_LABELS",
  "backend map targets must exactly cover the identity target count",
  "backend map target set is incomplete",
  'redirect: "manual"',
  'queryPrometheus(prometheusUrl, "time()"',
  "identity capture must predate the entire query window",
  "identity capture is older than the admitted maximum age",
  "must expose exactly one current admitted build-info series equal to 1",
  "count_over_time(rustok_page_builder_provider_build_info",
  'source_commit!="',
  "observed an unexpected source commit inside the query window",
  "multiple expected target ids resolve to the same current backend series",
  "rustok_page_builder_provider_last_observation_unix_seconds",
  "freshness is stale",
  "increase(rustok_page_builder_provider_operation_completed_total",
  "increase(rustok_page_builder_provider_operation_duration_seconds_bucket",
  "MINIMUM_SAMPLES_PER_OPERATION = 20",
  "histogramQuantile95",
  "sanitizeFailures / publishSamples",
  "runtimeFailures / (previewSamples + publishSamples)",
  "THRESHOLDS.preview_p95_ms",
  "THRESHOLDS.publish_p95_ms",
  "THRESHOLDS.runtime_error_rate_max * 2.0",
  "raw_prometheus_url_persisted: false",
  "raw_matcher_values_persisted: false",
  "pages_provider_health_observed: false",
]) need(sources.evaluator, marker, "deployment evaluator runner");

for (const marker of [
  "deployment-health-backend-evaluator-source-ready",
  "exact-target-source-admission-source-ready",
  "freshness-and-sample-floor-source-ready",
  "identity capture must predate the **entire** query window",
  "Partial success is rejected",
  "Prometheus `time()`",
  "preview terminal completions >= 20",
  "publish terminal completions >= 20",
  "summed cumulative bucket increases",
  "Pages remains `unobserved`",
  "retained identity/evaluator runtime evidence [maintainer execution pending]",
  "tests were not run",
]) need(sources.overlay, marker, "deployment evaluator overlay");

for (const marker of [
  "deployment-health-evaluator-source-ready",
  "page-builder-provider-health-deployment-evaluator-actualization-2026-08-09.md",
  "retained deployment health evaluator packet",
  "Pages remains `unobserved`",
]) need(sources.parity, marker, "plan parity actualization");

need(sources.pagesGraphql, "provider_health_observed: false", "Pages GraphQL remains unobserved");
forbid(sources.pagesGraphql, "provider_health_observed: true", "Pages GraphQL promotion");
need(sources.pagesFacade, "PageBuilderAdminProviderStatus::unobserved", "Pages admin remains unobserved");
forbid(sources.pagesGraphql, "deployment_evaluation", "Pages GraphQL must not bind evaluator packet yet");
forbid(sources.pagesFacade, "deployment_evaluation", "Pages admin must not bind evaluator packet yet");

if (contract.next_cursor?.deployment_health_backend_evaluator !== "source_ready_maintainer_execution_pending") {
  failures.push("deployment evaluator cursor must be source-ready and execution-pending");
}
if (contract.next_cursor?.deployment_health_runtime_evidence !== "maintainer_execution_pending") {
  failures.push("deployment health runtime evidence must remain maintainer execution pending");
}
if (
  contract.next_cursor?.pages_provider_status_binding !==
  "blocked_on_retained_deployment_evaluation_and_owner_acceptance"
) failures.push("Pages provider-status binding must remain blocked on retained evaluation and owner acceptance");

if (failures.length > 0) {
  console.error("[verify-page-builder-provider-health-deployment-evaluator] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "[verify-page-builder-provider-health-deployment-evaluator] PASS source_ready=true execution=pending pages_health=unobserved",
);
