#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const files = {
  builder: "crates/rustok-pages/admin/src/builder.rs",
  adapter: "crates/rustok-pages/admin/src/builder_rollout_settings.rs",
  composition: "crates/rustok-pages/admin/src/composition.rs",
  adminMain: "apps/admin/src/main.rs",
  evidence: "crates/rustok-pages/contracts/evidence/pages-builder-rollout-binding-source.json",
  healthBinding: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-consumer-binding-source.json",
  gate: "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json",
  actualization: "docs/modules/pages-page-builder-rollout-binding-actualization-2026-08-08.md",
  parity: "docs/modules/pages-page-builder-plan-parity-actualization-2026-08-08.md",
};
const failures = [];
const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
const need = (source, marker, label) => { if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`); };
const forbid = (source, marker, label) => { if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`); };

for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(absolute(relativePath))) { failures.push(`${label}: missing ${relativePath}`); continue; }
  const stats = fs.lstatSync(absolute(relativePath));
  if (!stats.isFile() || stats.isSymbolicLink()) failures.push(`${label}: ${relativePath} must be a regular non-symlink file`);
}
if (failures.length) {
  console.error("[verify-pages-builder-rollout-binding] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const sources = Object.fromEntries(Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]));
const evidence = JSON.parse(sources.evidence);
const healthBinding = JSON.parse(sources.healthBinding);
const gate = JSON.parse(sources.gate);

for (const marker of [
  "provider_status: Option<PageBuilderAdminProviderStatus>",
  "with_provider_status",
  "fetch_pages_builder_rollout_snapshot(",
  "trusted_rollout.effective_runtime_flags()",
  "compose_fly_page_builder_handlers(store, renderer, effective_flags)",
]) need(sources.builder, marker, "authoritative SSR binding");
for (const marker of [
  "fn pages_builder_capability_flags() -> BuilderCapabilityFlags",
  "load_trusted_pages_builder_rollout_snapshot()",
  "compose_fly_page_builder_handlers(store, renderer, trusted_rollout.flags)",
]) forbid(sources.builder, marker, "superseded rollout binding");

for (const marker of [
  "PagesBuilderRolloutSnapshot",
  "fetch_pages_builder_rollout_snapshot(",
  "PagesBuilderRolloutSnapshotError::TenantMismatch",
  "pub fn provider_status(&self) -> PageBuilderAdminProviderStatus",
  "pub fn effective_runtime_flags(&self) -> BuilderCapabilityFlags",
  "pages_editor_capabilities_for_snapshot(",
]) need(sources.adapter, marker, "stateless rollout/health adapter");
for (const marker of ["HostRuntimeContext", "tenant_module_settings(", "leptos_axum::extract"])
  forbid(sources.adapter, marker, "admin rollout adapter");

for (const marker of [
  "fetch_pages_builder_rollout_snapshot(",
  ".provider_status();",
  "provider_status: PageBuilderAdminProviderStatus",
  ".with_provider_status(provider_status)",
]) need(sources.composition, marker, "workspace binding");

for (const marker of [
  "fetch_pages_builder_rollout_snapshot(",
  "pages_editor_capabilities_for_snapshot(",
  "dispatch_pages_browser_intent_with_capabilities(snapshot, envelope, editor_capabilities)",
]) need(sources.adminMain, marker, "browser-intent preflight binding");

if (evidence.format !== "pages_builder_rollout_binding_source_v1") failures.push(`evidence format drifted: ${evidence.format}`);
if (evidence.status !== "server_owner_ui_ssr_and_browser_intent_binding_source_ready_runtime_matrix_pending") failures.push(`evidence status drifted: ${evidence.status}`);
for (const [key, expected] of Object.entries({
  ui_workspace_loads_server_owned_snapshot: true,
  ui_facade_provider_status_uses_loaded_flags: true,
  ui_facade_provider_health_uses_validated_snapshot: true,
  authoritative_ssr_rereads_server_owned_snapshot_per_request: true,
  authoritative_ssr_verifies_snapshot_tenant_slug: true,
  authoritative_ssr_composes_handlers_from_health_limited_flags: true,
  standalone_browser_intent_fetches_server_owned_snapshot: true,
  standalone_browser_intent_narrows_role_capabilities_from_validated_snapshot: true,
  provider_health_only_narrows_rollout_and_role_capabilities: true,
  browser_rollout_flags_are_authoritative: false,
  standalone_admin_host_runtime_context_required: false,
  hardcoded_all_on_consumer_binding_present: false,
  four_profile_runtime_evidence_retained: false,
  observed_provider_health_runtime_evidence_retained: false,
  gate_accepted: false,
})) {
  if (evidence.source_contract?.[key] !== expected) failures.push(`evidence source_contract.${key} must be ${expected}`);
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) if (value !== false) failures.push(`evidence validation.${key} must remain false`);

if (healthBinding.format !== "pages_builder_provider_health_consumer_binding_source_v1") {
  failures.push("health consumer binding source must be present");
}
if (healthBinding.status !== "source_ready_runtime_activation_pending") {
  failures.push("health consumer binding runtime activation must remain pending");
}

if (gate.accepted !== false) failures.push("Pages reference-consumer gate must remain unaccepted");
if (gate.current_boundary?.four_profile_runtime_matrix !== "harness_source_ready_maintainer_execution_pending") failures.push("runtime matrix harness must remain maintainer-execution pending");
if (gate.current_boundary?.provider_health !== "unobserved") failures.push("retained gate provider health must remain unobserved until runtime evidence is executed");

for (const marker of [
  "server-owner-snapshot-source-ready",
  "browser-intent-preflight-binding-source-ready",
  "runtime-matrix-evidence-pending",
  "No tests, Node verifiers, Cargo commands",
]) need(sources.actualization, marker, "rollout actualization");
for (const marker of [
  "provider-health-consumer-binding-source-ready",
  "UI / SSR / browser-intent provider-health binding [source-ready]",
]) need(sources.parity, marker, "current parity actualization");

if (failures.length) {
  console.error("[verify-pages-builder-rollout-binding] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-builder-rollout-binding] PASS bindings=ui+ssr+browser_intent rollout=server_owned health=source_ready runtime_evidence=pending");
