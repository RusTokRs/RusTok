#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  health: "crates/modules/rustok-page-builder/src/health.rs",
  telemetry: "crates/modules/rustok-page-builder/src/runtime_telemetry.rs",
  composition: "crates/modules/rustok-page-builder/src/composition.rs",
  flyService: "crates/modules/rustok-page-builder/src/adapters/fly_service.rs",
  pagesGraphql: "crates/modules/rustok-pages/src/graphql/builder_rollout.rs",
  pagesFacade: "crates/modules/rustok-pages/admin/src/builder.rs",
  evidence: "crates/modules/rustok-page-builder/contracts/evidence/page-builder-provider-health-runtime-observation-source.json",
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
  console.error("[verify-page-builder-provider-health-runtime-observation] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);

if (evidence.format !== "page_builder_provider_health_runtime_observation_source_v1") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "source_ready_unvalidated") failures.push("evidence status drifted");
if (evidence.scope !== "process_local_runtime_observation") failures.push("evidence scope drifted");
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}

const contract = evidence.source_contract ?? {};
for (const [key, expected] of Object.entries({
  default_fly_composition_records_runtime_health: true,
  preview_source_operation: "render_preview",
  publish_source_operation: "save_project",
  load_project_excluded: true,
  window_capacity_per_operation: 256,
  minimum_preview_samples: 20,
  minimum_publish_samples: 20,
  snapshot_absent_below_sample_floor: true,
  process_restart_resets_window: true,
  provider_health_snapshot_uses_existing_pilot_thresholds: true,
  terminal_success_and_failure_are_recorded: true,
  sanitize_failure_rate_uses_telemetry_visible_publish_failures_only: true,
  runtime_error_rate_uses_telemetry_visible_preview_and_publish_failures_only: true,
  pre_telemetry_validation_and_inspection_are_not_included: true,
  bounded_pending_call_correlation: true,
  deployment_wide_aggregation_present: false,
  deployment_freshness_contract_present: false,
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

, marker, "runtime observation overlay");

, marker, "plan parity actualization");

if (evidence.next_cursor?.deployment_aggregation_and_freshness_contract !== "open") {
  failures.push("next cursor must keep deployment aggregation/freshness open");
}
if (evidence.next_cursor?.pages_provider_status_binding !== "blocked_on_deployment_observation_authority") {
  failures.push("Pages provider status binding must remain blocked");
}
if (evidence.next_cursor?.observed_provider_health_gate !== "open") {
  failures.push("observed provider health gate must remain open");
}

if (failures.length > 0) {
  console.error("[verify-page-builder-provider-health-runtime-observation] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log("[verify-page-builder-provider-health-runtime-observation] PASS source_ready=true deployment_health=open execution=pending");
