#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  status: "crates/rustok-page-builder/admin/src/provider_status.rs",
  facade: "crates/rustok-page-builder/admin/src/transport/mod.rs",
  canvas: "crates/rustok-page-builder/admin/src/editor/modular_canvas.rs",
  policyPanel: "crates/rustok-page-builder/admin/src/editor/capability_controls.rs",
  preview: "crates/rustok-page-builder/admin/src/editor/server_preview.rs",
  enLocale: "crates/rustok-page-builder/admin/locales/en.json",
  ruLocale: "crates/rustok-page-builder/admin/locales/ru.json",
  pagesFacade: "crates/rustok-pages/admin/src/builder.rs",
  evidence: "crates/rustok-page-builder/contracts/evidence/page-builder-admin-provider-status-source.json",
  overlay: "docs/modules/page-builder-provider-degraded-controls-actualization-2026-08-07.md",
  localPlan: "crates/rustok-page-builder/docs/implementation-plan.md",
  centralPlan: "docs/modules/page-builder-implementation-plan.md",
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
  console.error("[verify-page-builder-admin-provider-status] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);
if (evidence.format !== "page_builder_admin_provider_status_source_v1") failures.push("evidence format drifted");
if (evidence.status !== "page_builder_admin_provider_status_source_unvalidated") failures.push("evidence status drifted");
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) failures.push("execution evidence must remain empty");
for (const value of Object.values(evidence.validation ?? {})) {
  if (value !== false) failures.push("validation fields must remain false");
}
for (const key of [
  "admin_facade_provider_status_is_optional",
  "provider_status_carries_rollout_flags",
  "provider_status_health_snapshot_is_optional",
  "missing_health_is_reported_as_unobserved_not_healthy",
  "provider_status_only_narrows_host_capabilities",
  "invalid_rollout_flags_fail_closed_to_unavailable",
  "builder_disabled_forces_read_only",
  "observed_unavailable_health_forces_read_only",
  "degraded_provider_disables_publish",
  "publish_disabled_rollout_disables_publish",
  "properties_disabled_rollout_disables_properties",
  "preview_disabled_rollout_disables_server_preview",
  "server_preview_click_path_rechecks_provider_status",
  "pages_facade_reports_the_same_rollout_flags_used_by_server_composition",
  "pages_health_remains_unobserved_without_live_slo_snapshot",
  "fallback_editor_is_not_added",
]) {
  if (evidence.source_contract?.[key] !== true) failures.push(`source_contract.${key} must be true`);
}
for (const key of ["tests_run", "static_verifier_run", "cargo_run", "formatting_run", "browser_run", "workflows_or_ci_run"]) {
  if (evidence.source_contract?.[key] !== false) failures.push(`source_contract.${key} must remain false`);
}

for (const marker of [
  "pub enum PageBuilderAdminProviderState",
  "Unobserved",
  "pub struct PageBuilderAdminProviderStatus",
  "pub flags: BuilderCapabilityFlags",
  "pub health: Option<ProviderHealthSnapshot>",
  "self.flags.validate().is_err()",
  "!self.flags.builder_enabled",
  "ProviderHealthState::Unavailable",
  "ProviderHealthState::Degraded",
  "!self.flags.properties_enabled",
  "!self.flags.publish_enabled",
  "pub fn preview_enabled",
  "pub fn limit_capabilities",
  "CapabilityState::read_only()",
]) need(sources.status, marker, "provider status contract");

for (const marker of [
  "unobserved_all_on_status_does_not_claim_healthy_or_reduce_capabilities",
  "rollout_publish_off_is_degraded_and_removes_publish_only",
  "rollout_preview_off_is_degraded_and_keeps_properties_without_preview_or_publish",
  "rollout_builder_off_is_unavailable_and_forces_read_only",
]) need(sources.status, marker, "reference gate provider-profile tests");

for (const marker of [
  "fn provider_status(&self) -> Option<PageBuilderAdminProviderStatus>",
  "self.as_ref().provider_status()",
  "returning no snapshot is intentionally different from claiming the provider",
]) need(sources.facade, marker, "admin facade seam");

for (const marker of [
  "facade.provider_status()",
  "status.limit_capabilities(capabilities)",
  "UiIntent::SetEditableCapabilities(capabilities)",
  "provider_status=server_preview_provider_status",
  "provider_status=capability_provider_status",
]) need(sources.canvas, marker, "admin canvas provider narrowing");

for (const marker of [
  "Provider control state",
  "Observed health",
  "Host provider policy",
  "Rollout flags",
  "Degradation reasons",
  "data-fly-provider-control-state",
  "PageBuilderAdminProviderState::Unobserved",
]) need(sources.policyPanel, marker, "provider policy panel");
for (const marker of [
  "status.preview_enabled()",
  "Server preview is unavailable under the current Page Builder provider status",
  "data-page-builder-provider-preview",
]) need(sources.preview, marker, "server preview provider gate");
for (const marker of [
  '"providerControl"',
  '"observedHealth"',
  '"hostProviderPolicy"',
  '"rollout"',
  '"degradationReasons"',
  '"unobserved"',
]) {
  need(sources.enLocale, marker, "English provider status locale");
  need(sources.ruLocale, marker, "Russian provider status locale");
}

for (const marker of [
  "fn pages_builder_capability_flags() -> BuilderCapabilityFlags",
  "fn provider_status(&self) -> Option<PageBuilderAdminProviderStatus>",
  "PageBuilderAdminProviderStatus::unobserved",
  "compose_fly_page_builder_handlers(store, renderer, pages_builder_capability_flags())",
]) need(sources.pagesFacade, marker, "Pages facade/composition flag identity");
for (const marker of [
  "Page Builder Provider Degraded Controls Actualization",
  "unobserved",
  "No fallback editor is mounted",
  "Execution remains pending",
]) need(sources.overlay, marker, "provider status actualization");
for (const marker of ["admin-provider-status-source-ready", "provider-health", "observed health"]) need(sources.localPlan, marker, "local plan actualization");
for (const marker of ["Provider status/degraded controls", "observed provider-health evidence", "next production consumer"]) need(sources.centralPlan, marker, "central plan actualization");

for (const marker of ["fallback editor", "ProviderHealthSnapshot::evaluate(ProviderSloObservations::default())"]) {
  forbid(sources.canvas, marker, "admin canvas must not fabricate fallback/health");
  forbid(sources.pagesFacade, marker, "Pages facade must not fabricate fallback/health");
}

if (failures.length > 0) {
  console.error("[verify-page-builder-admin-provider-status] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-page-builder-admin-provider-status] PASS source_ready=true execution=pending");
