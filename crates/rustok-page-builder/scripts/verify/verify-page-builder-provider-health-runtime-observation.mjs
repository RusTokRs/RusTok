#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  health: "crates/rustok-page-builder/src/health.rs",
  telemetry: "crates/rustok-page-builder/src/runtime_telemetry.rs",
  composition: "crates/rustok-page-builder/src/composition.rs",
  flyService: "crates/rustok-page-builder/src/adapters/fly_service.rs",
  pagesGraphql: "crates/rustok-pages/src/graphql/builder_rollout.rs",
  pagesFacade: "crates/rustok-pages/admin/src/builder.rs",
  evidence: "crates/rustok-page-builder/contracts/evidence/page-builder-provider-health-runtime-observation-source.json",
  overlay: "docs/modules/page-builder-provider-health-runtime-observation-actualization-2026-08-09.md",
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

for (const marker of [
  "pub const PROVIDER_HEALTH_WINDOW_CAPACITY: usize = 256;",
  "pub const PROVIDER_HEALTH_MINIMUM_SAMPLES_PER_OPERATION: usize = 20;",
  "pub enum ProviderHealthOperation",
  "Preview,",
  "Publish,",
  "pub enum ProviderHealthOutcome",
  "pub fn record_provider_health_observation",
  "pub fn provider_health_runtime_snapshot() -> Option<ProviderHealthSnapshot>",
  "window.preview.len() < PROVIDER_HEALTH_MINIMUM_SAMPLES_PER_OPERATION",
  "window.publish.len() < PROVIDER_HEALTH_MINIMUM_SAMPLES_PER_OPERATION",
  "ProviderHealthSnapshot::evaluate(ProviderSloObservations",
  "process-local provider-health snapshot",
]) need(sources.health, marker, "health runtime window");

for (const marker of [
  "const PROVIDER_HEALTH_PENDING_CALL_CAPACITY: usize = 1024;",
  "pub struct ProviderHealthRuntimeTelemetry",
  "PageBuilderRuntimeOperation::RenderPreview => Some(ProviderHealthOperation::Preview)",
  "PageBuilderRuntimeOperation::SaveProject => Some(ProviderHealthOperation::Publish)",
  "PageBuilderRuntimeOperation::LoadProject => None",
  "Some(PageBuilderErrorKind::Sanitize) => ProviderHealthOutcome::SanitizeFailed",
  "Some(PageBuilderErrorKind::Runtime) => ProviderHealthOutcome::RuntimeFailed",
  "record_provider_health_observation(operation, pending_call.started_at.elapsed(), outcome)",
]) need(sources.telemetry, marker, "runtime telemetry observer");

for (const marker of [
  "T = ProviderHealthRuntimeTelemetry",
  "FlyAdapterBackedPageBuilderService::with_telemetry",
  "ProviderHealthRuntimeTelemetry::default()",
  "process-local Preview/Publish runtime observations",
]) need(sources.composition, marker, "default composition");

for (const marker of [
  "PageBuilderRuntimeCallEvidence::render_preview",
  "self.telemetry.record_runtime_call(&evidence);",
  "self.telemetry.record_runtime_call(&evidence.succeeded());",
  "PageBuilderRuntimeCallEvidence::save_project",
  "self.telemetry.record_runtime_call(&evidence.failed(&error));",
]) need(sources.flyService, marker, "Fly terminal telemetry seam");

// This first slice must not promote process-local observations into authoritative Pages health.
need(sources.pagesGraphql, "provider_health_observed: false", "Pages GraphQL remains unobserved");
need(sources.pagesFacade, "PageBuilderAdminProviderStatus::unobserved", "Pages admin remains unobserved");
forbid(sources.pagesGraphql, "provider_health_runtime_snapshot()", "Pages GraphQL must not consume process-local health yet");
forbid(sources.pagesFacade, "provider_health_runtime_snapshot()", "Pages admin must not consume process-local health yet");

for (const marker of [
  "process-local-runtime-observation-source-ready",
  "deployment-observed-health-open",
  "20 Preview",
  "20 Publish",
  "256",
  "provider_health_observed = false",
  "deployment-wide",
  "tests were not run",
]) need(sources.overlay, marker, "runtime observation overlay");

for (const marker of [
  "provider-runtime-observation-source-ready",
  "deployment-observed-health-open",
  "page-builder-provider-health-runtime-observation-actualization-2026-08-09.md",
  "Pages remains `unobserved`",
]) need(sources.parity, marker, "plan parity actualization");

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
