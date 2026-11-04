#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const paths = {
  health: "crates/modules/rustok-page-builder/src/health.rs",
  telemetry: "crates/libs/rustok-telemetry/src/page_builder_provider_metrics.rs",
  identity: "crates/modules/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-identity-source.json",
  contract: "crates/modules/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-evaluator-source.json",
  evaluator: "scripts/evidence/evaluate-page-builder-provider-health-deployment.mjs",
  pagesGraphql: "crates/modules/rustok-pages/src/graphql/builder_rollout.rs",
  pagesFacade: "crates/modules/rustok-pages/admin/src/builder_rollout_settings.rs",
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

  , marker, "evaluator overlay");

  , marker, "parity actualization");

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
