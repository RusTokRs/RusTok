#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const paths = {
  health: "crates/rustok-page-builder/src/health.rs",
  telemetry: "crates/rustok-telemetry/src/page_builder_provider_metrics.rs",
  identity: "crates/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-identity-source.json",
  contract: "crates/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-evaluator-source.json",
  evaluator: "scripts/evidence/evaluate-page-builder-provider-health-deployment.mjs",
  overlay: "docs/modules/page-builder-provider-health-deployment-evaluator-actualization-2026-08-09.md",
  parity: "docs/modules/pages-page-builder-plan-parity-actualization-2026-08-08.md",
  pagesGraphql: "crates/rustok-pages/src/graphql/builder_rollout.rs",
  pagesFacade: "crates/rustok-pages/admin/src/builder.rs",
};

function load(label) {
  const relative = paths[label];
  const absolute = path.join(repoRoot, relative);
  if (!fs.existsSync(absolute)) {
    failures.push(`${label}: missing ${relative}`);
    return "";
  }
  const stats = fs.lstatSync(absolute);
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${label}: ${relative} must be a regular non-symlink file`);
    return "";
  }
  return fs.readFileSync(absolute, "utf8");
}

function need(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function forbid(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

const sources = Object.fromEntries(Object.keys(paths).map((label) => [label, load(label)]));
if (failures.length === 0) {
  let contract;
  try {
    contract = JSON.parse(sources.contract);
  } catch (error) {
    failures.push(`contract: invalid JSON: ${error.message}`);
    contract = {};
  }

  if (contract.format !== "page_builder_provider_health_deployment_evaluator_source_v1") {
    failures.push("contract format drifted");
  }
  if (contract.status !== "source_ready_execution_pending") {
    failures.push("contract must remain source_ready_execution_pending");
  }

  const expectedContractValues = [
    [contract.identity_input, "format", "page_builder_provider_health_deployment_identity_v1"],
    [contract.identity_input, "required_status", "deployment_identity_verified_health_evaluation_pending"],
    [contract.identity_input, "source_commit_must_equal_checkout_head", true],
    [contract.identity_input, "inventory_complete_must_be_true", true],
    [contract.backend_target_map, "authority", "maintainer_supplied_complete_prometheus_target_mapping"],
    [contract.backend_target_map, "target_id_set_must_equal_identity_packet", true],
    [contract.backend_target_map, "target_label_values_unique", true],
    [contract.backend_target_map, "regex_matchers_allowed", false],
    [contract.backend_query, "prometheus_api", "/api/v1/query"],
    [contract.backend_query, "request_method", "POST"],
    [contract.backend_query, "redirects_followed", false],
    [contract.backend_query, "maximum_parallel_queries", 8],
    [contract.backend_query, "query_window_seconds_minimum", 300],
    [contract.backend_query, "query_window_seconds_maximum", 86400],
    [contract.backend_query, "freshness_seconds_minimum", 60],
    [contract.backend_query, "identity_capture_must_predate_entire_query_window", true],
    [contract.backend_query, "identity_capture_maximum_age_seconds", 86400],
    [contract.backend_query, "backend_clock_source", "prometheus_time_function"],
    [contract.source_admission, "unexpected_source_build_info_in_query_window", "fail_closed"],
    [contract.source_admission, "partial_target_query_success", "fail_closed"],
    [contract.freshness_admission, "preview_required_per_target", true],
    [contract.freshness_admission, "publish_required_per_target", true],
    [contract.freshness_admission, "stale_operation_freshness", "fail_closed"],
    [contract.aggregation, "counter_reset_aware_function", "increase"],
    [contract.aggregation, "histogram_quantile", 0.95],
    [contract.aggregation, "histogram_completion_population_must_match", true],
    [contract.aggregation, "population_consistency_tolerance_fraction", 0.000001],
    [contract.aggregation, "minimum_preview_samples", 20],
    [contract.aggregation, "minimum_publish_samples", 20],
    [contract.provider_health_policy, "preview_p95_ms_max", 1500],
    [contract.provider_health_policy, "publish_p95_ms_max", 3000],
    [contract.provider_health_policy, "sanitize_failure_rate_max", 0.01],
    [contract.provider_health_policy, "runtime_error_rate_max", 0.01],
    [contract.provider_health_policy, "unavailable_runtime_error_multiplier", 2.0],
    [contract.non_claims, "runtime_evaluation_executed", false],
    [contract.non_claims, "pages_provider_health_observed", false],
    [contract.non_claims, "pages_reference_consumer_gate_accepted", false],
    [contract.non_claims, "forum_wave_accepted", false],
    [contract.non_claims, "tests_run", false],
    [contract.next_cursor, "deployment_health_backend_evaluator", "source_ready_maintainer_execution_pending"],
    [contract.next_cursor, "deployment_health_runtime_evidence", "maintainer_execution_pending"],
  ];
  for (const [object, key, expected] of expectedContractValues) {
    if (object?.[key] !== expected) failures.push(`${key} must equal ${JSON.stringify(expected)}`);
  }

  for (const reserved of ["__name__", "source_commit", "operation", "outcome", "le"]) {
    if (!(contract.backend_target_map?.reserved_matcher_labels ?? []).includes(reserved)) {
      failures.push(`reserved matcher label missing: ${reserved}`);
    }
  }

  for (const marker of [
    "pub const PROVIDER_HEALTH_MINIMUM_SAMPLES_PER_OPERATION: usize = 20;",
    "preview_p95_ms: 1500",
    "publish_p95_ms: 3000",
    "sanitize_failure_rate_max: 0.01",
    "runtime_error_rate_max: 0.01",
    "thresholds.runtime_error_rate_max * 2.0",
  ]) need(sources.health, marker, "Rust health policy");

  for (const marker of [
    "rustok_page_builder_provider_build_info",
    "rustok_page_builder_provider_operation_duration_seconds",
    "rustok_page_builder_provider_operation_completed_total",
    "rustok_page_builder_provider_last_observation_unix_seconds",
  ]) need(sources.telemetry, marker, "provider metrics");

  for (const marker of [
    '"deployment_health_backend_evaluator": "open"',
    '"deployment_identity_capture": "maintainer_execution_pending"',
  ]) need(sources.identity, marker, "identity predecessor");

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
    "selectorExcludingSource",
    "observed an unexpected source commit inside the query window",
    "multiple expected target ids resolve to the same current backend series",
    "rustok_page_builder_provider_last_observation_unix_seconds",
    "freshness is stale",
    "increase(rustok_page_builder_provider_operation_completed_total",
    "increase(rustok_page_builder_provider_operation_duration_seconds_bucket",
    "requireHistogramPopulationConsistency",
    "histogram +Inf population does not match terminal completion population",
    "MINIMUM_SAMPLES_PER_OPERATION = 20",
    "histogramQuantile95",
    "sanitizeFailures / publishSamples",
    "runtimeFailures / (previewSamples + publishSamples)",
    "THRESHOLDS.runtime_error_rate_max * 2.0",
    "raw_prometheus_url_persisted: false",
    "raw_matcher_values_persisted: false",
    "pages_provider_health_observed: false",
  ]) need(sources.evaluator, marker, "evaluator runner");

  for (const marker of [
    "deployment-health-backend-evaluator-source-ready",
    "exact-target-source-admission-source-ready",
    "freshness-and-sample-floor-source-ready",
    "identity capture must predate the **entire** query window",
    "partial target success is rejected",
    "Prometheus `time()`",
    "preview terminal completions >= 20",
    "publish terminal completions >= 20",
    "summed cumulative bucket increases",
    "Pages remains `unobserved`",
    "retained identity/evaluator runtime evidence [maintainer execution pending]",
    "tests were not run",
  ]) need(sources.overlay, marker, "evaluator overlay");

  for (const marker of [
    "deployment-health-evaluator-source-ready",
    "page-builder-provider-health-deployment-evaluator-actualization-2026-08-09.md",
    "retained deployment health evaluator packet",
    "Pages remains `unobserved`",
  ]) need(sources.parity, marker, "parity actualization");

  need(sources.pagesGraphql, "provider_health_observed: false", "Pages GraphQL remains unobserved");
  forbid(sources.pagesGraphql, "provider_health_observed: true", "Pages GraphQL promotion");
  need(sources.pagesFacade, "PageBuilderAdminProviderStatus::unobserved", "Pages admin remains unobserved");
  forbid(sources.pagesGraphql, "deployment_evaluation", "Pages GraphQL evaluator binding");
  forbid(sources.pagesFacade, "deployment_evaluation", "Pages admin evaluator binding");
}

if (failures.length > 0) {
  console.error("[verify-page-builder-provider-health-deployment-evaluator] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "[verify-page-builder-provider-health-deployment-evaluator] PASS source_ready=true execution=pending pages_health=unobserved",
);
