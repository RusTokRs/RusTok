#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  telemetryMetrics: "crates/rustok-telemetry/src/page_builder_provider_metrics.rs",
  telemetryLib: "crates/rustok-telemetry/src/lib.rs",
  pageBuilderCargo: "crates/rustok-page-builder/Cargo.toml",
  runtimeTelemetry: "crates/rustok-page-builder/src/runtime_telemetry.rs",
  pagesGraphql: "crates/rustok-pages/src/graphql/builder_rollout.rs",
  pagesFacade: "crates/rustok-pages/admin/src/builder.rs",
  evidence: "crates/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-metrics-source.json",
  overlay: "docs/modules/page-builder-provider-health-deployment-metrics-actualization-2026-08-09.md",
  parity: "docs/modules/pages-page-builder-plan-parity-actualization-2026-08-08.md",
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
  console.error("[verify-page-builder-provider-health-deployment-metrics] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);

if (evidence.format !== "page_builder_provider_health_deployment_metrics_source_v1") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "source_ready_unvalidated") failures.push("evidence status drifted");
if (evidence.scope !== "deployment_aggregatable_prometheus_metrics") failures.push("evidence scope drifted");
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}

const contract = evidence.source_contract ?? {};
for (const [key, expected] of Object.entries({
  platform_registry_owner: "rustok-telemetry",
  default_fly_runtime_exports_metrics: true,
  duration_metric: "rustok_page_builder_provider_operation_duration_seconds",
  completion_metric: "rustok_page_builder_provider_operation_completed_total",
  freshness_metric: "rustok_page_builder_provider_last_observation_unix_seconds",
  duration_histogram_contains_preview_threshold_seconds: 1.5,
  duration_histogram_contains_publish_threshold_seconds: 3.0,
  tenant_label_present: false,
  page_label_present: false,
  revision_label_present: false,
  correlation_label_present: false,
  deployment_label_present_in_application_metric: false,
  scrape_target_identity_owned_by_metrics_backend: true,
  counter_reset_aware_range_aggregation_required: true,
  freshness_requires_both_operations: true,
  missing_or_stale_expected_target_fails_closed: true,
  expected_target_inventory_present: false,
  exact_source_deployment_identity_present: false,
  pages_admin_health_binding_present: false,
  provider_health_observed_promoted: false,
  pages_reference_consumer_gate_promoted: false,
  ffa_or_fba_promoted: false,
  tests_run: false,
  static_verifier_run: false,
  cargo_run: false,
  formatting_run: false,
  runtime_execution_run: false,
  workflows_or_ci_run: false,
})) {
  if (contract[key] !== expected) failures.push(`source_contract.${key} must equal ${JSON.stringify(expected)}`);
}

for (const marker of [
  'pub const PAGE_BUILDER_PROVIDER_OPERATIONS: [&str; 2] = ["preview", "publish"]',
  '"succeeded"',
  '"sanitize_failed"',
  '"runtime_failed"',
  '"other_failed"',
  'rustok_page_builder_provider_operation_duration_seconds',
  'rustok_page_builder_provider_operation_completed_total',
  'rustok_page_builder_provider_last_observation_unix_seconds',
  '1.5',
  '3.0',
  '&["operation"]',
  '&["operation", "outcome"]',
  'pub fn record_page_builder_provider_operation',
]) need(sources.telemetryMetrics, marker, "platform Page Builder metrics");
for (const forbidden of [
  '"tenant_id"',
  '"page_id"',
  '"revision_id"',
  '"correlation_id"',
  '&["deployment"]',
  '&["instance"]',
]) forbid(sources.telemetryMetrics, forbidden, "platform Page Builder metrics cardinality");

need(sources.telemetryLib, "pub mod page_builder_provider_metrics;", "telemetry module registration");
need(sources.telemetryLib, "page_builder_provider_metrics::register(registry)?;", "telemetry registry registration");
need(sources.pageBuilderCargo, '"dep:rustok-telemetry"', "Page Builder server feature");
need(sources.pageBuilderCargo, "rustok-telemetry = { workspace = true, optional = true }", "Page Builder telemetry dependency");

for (const marker of [
  "record_page_builder_provider_operation",
  'ProviderHealthOperation::Preview => "preview"',
  'ProviderHealthOperation::Publish => "publish"',
  'ProviderHealthOutcome::Succeeded => "succeeded"',
  'ProviderHealthOutcome::SanitizeFailed => "sanitize_failed"',
  'ProviderHealthOutcome::RuntimeFailed => "runtime_failed"',
  'ProviderHealthOutcome::OtherFailed => "other_failed"',
  "let elapsed = pending_call.started_at.elapsed();",
  "record_provider_health_observation(operation, elapsed, outcome);",
]) need(sources.runtimeTelemetry, marker, "Page Builder runtime export");

need(sources.pagesGraphql, "provider_health_observed: false", "Pages GraphQL remains unobserved");
need(sources.pagesFacade, "PageBuilderAdminProviderStatus::unobserved", "Pages admin remains unobserved");
forbid(sources.pagesGraphql, "page_builder_provider_metrics", "Pages GraphQL must not consume raw metrics");
forbid(sources.pagesFacade, "page_builder_provider_metrics", "Pages admin must not consume raw metrics");

for (const marker of [
  "deployment-aggregatable-metrics-source-ready",
  "freshness-signal-source-ready",
  "exact-deployment-identity-open",
  "rate(rustok_page_builder_provider_operation_duration_seconds_bucket",
  "increase(rustok_page_builder_provider_operation_completed_total",
  "missing or stale",
  "Pages remains `unobserved`",
  "tests were not run",
]) need(sources.overlay, marker, "deployment metrics overlay");

for (const marker of [
  "deployment-metrics-source-ready",
  "freshness-signal-source-ready",
  "page-builder-provider-health-deployment-metrics-actualization-2026-08-09.md",
  "exact source/deployment identity",
  "Pages remains `unobserved`",
]) need(sources.parity, marker, "plan parity actualization");

if (evidence.next_cursor?.exact_source_deployment_identity !== "open") {
  failures.push("next cursor must keep exact source/deployment identity open");
}
if (evidence.next_cursor?.expected_target_inventory !== "open") {
  failures.push("next cursor must keep expected target inventory open");
}
if (
  evidence.next_cursor?.pages_provider_status_binding !==
  "blocked_on_exact_deployment_observation_authority"
) failures.push("Pages provider status binding must remain blocked");

if (failures.length > 0) {
  console.error("[verify-page-builder-provider-health-deployment-metrics] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log("[verify-page-builder-provider-health-deployment-metrics] PASS source_ready=true exact_deployment_identity=open execution=pending");
