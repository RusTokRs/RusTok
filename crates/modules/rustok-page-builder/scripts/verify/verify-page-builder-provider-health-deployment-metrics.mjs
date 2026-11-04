#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  telemetryMetrics: "crates/libs/rustok-telemetry/src/page_builder_provider_metrics.rs",
  telemetryLib: "crates/libs/rustok-telemetry/src/lib.rs",
  pageBuilderCargo: "crates/modules/rustok-page-builder/Cargo.toml",
  runtimeTelemetry: "crates/modules/rustok-page-builder/src/runtime_telemetry.rs",
  pagesGraphql: "crates/modules/rustok-pages/src/graphql/builder_rollout.rs",
  pagesFacade: "crates/modules/rustok-pages/admin/src/builder.rs",
  evidence: "crates/modules/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-metrics-source.json",
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

, marker, "deployment metrics overlay");

, marker, "plan parity actualization");

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
