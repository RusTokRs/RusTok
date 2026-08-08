#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const files = {
  rollout: "crates/rustok-page-builder/src/rollout.rs",
  owner: "crates/rustok-pages/src/graphql/builder_rollout.rs",
  adapter: "crates/rustok-pages/admin/src/builder_rollout_settings.rs",
  transport: "crates/rustok-pages/admin/src/transport/builder_rollout_adapter.rs",
  builder: "crates/rustok-pages/admin/src/builder.rs",
  composition: "crates/rustok-pages/admin/src/composition.rs",
  adminMain: "apps/admin/src/main.rs",
  cargo: "crates/rustok-pages/admin/Cargo.toml",
  evidence: "crates/rustok-pages/contracts/evidence/pages-builder-rollout-server-snapshot-source.json",
  gate: "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json",
  actualization: "docs/modules/pages-page-builder-rollout-server-snapshot-actualization-2026-08-08.md",
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
  console.error("[verify-pages-builder-rollout-server-snapshot] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const sources = Object.fromEntries(Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]));
const evidence = JSON.parse(sources.evidence);
const gate = JSON.parse(sources.gate);

for (const marker of [
  "pub fn from_module_settings(settings: &Value)",
  "module_settings_normalization_matches_all_declared_profiles",
  "module_settings_normalization_defaults_missing_keys_but_rejects_bad_types",
]) need(sources.rollout, marker, "shared rollout normalizer");

for (const marker of [
  "page_builder_rollout_snapshot",
  "require_module_enabled(ctx, MODULE_SLUG).await?",
  "auth.tenant_id != tenant.id",
  "Permission::PAGES_READ",
  "tenant_module_settings(db, tenant.id, MODULE_SLUG)",
  "BuilderCapabilityFlags::from_module_settings(&settings)",
  "provider_health_observed: false",
]) need(sources.owner, marker, "Pages server rollout owner");

for (const marker of [
  "fetch_pages_builder_rollout_snapshot(",
  "PagesBuilderRolloutSnapshotError::TenantMismatch",
  "pages_editor_capabilities_for_rollout(",
]) need(sources.adapter, marker, "stateless admin rollout adapter");
for (const marker of ["HostRuntimeContext", "tenant_module_settings(", "leptos_axum::extract"])
  forbid(sources.adapter, marker, "admin rollout adapter must not own DB request context");

for (const marker of [
  "pageBuilderRolloutSnapshot",
  "providerHealthObserved",
  "payload.provider_health_observed",
  "flags.validate()",
]) need(sources.transport, marker, "GraphQL rollout transport");

for (const marker of [
  "fetch_pages_builder_rollout_snapshot(",
  "compose_fly_page_builder_handlers(store, renderer, trusted_rollout.flags)",
]) need(sources.builder, marker, "authoritative SSR binding");
need(sources.composition, "fetch_pages_builder_rollout_snapshot(", "workspace binding");
for (const marker of [
  "fetch_pages_builder_rollout_snapshot(",
  "pages_editor_capabilities_for_rollout(",
  "dispatch_pages_browser_intent_with_capabilities(snapshot, envelope, editor_capabilities)",
]) need(sources.adminMain, marker, "standalone browser-intent binding");

forbid(sources.cargo, "dep:leptos_axum", "Pages admin rollout dependencies");
forbid(sources.cargo, "rustok-api/server", "Pages admin rollout dependencies");

if (evidence.format !== "pages_builder_rollout_server_snapshot_source_v1") failures.push(`evidence format drifted: ${evidence.format}`);
if (evidence.status !== "server_owner_graphql_snapshot_source_ready_all_admin_bindings_complete_runtime_evidence_pending") failures.push(`evidence status drifted: ${evidence.status}`);
for (const [key, expected] of Object.entries({
  tenant_context_is_server_resolved: true,
  auth_context_is_server_resolved: true,
  auth_tenant_must_match_routed_tenant: true,
  pages_read_authority_required: true,
  raw_settings_are_not_returned: true,
  provider_health_observed_returned_false: true,
  standalone_admin_host_runtime_context_required: false,
  browser_supplied_flags_accepted: false,
  pages_ui_facade_binding_complete: true,
  pages_ssr_dispatch_binding_complete: true,
  standalone_browser_intent_preflight_binding_complete: true,
  four_profile_runtime_evidence_retained: false,
  provider_health_observed: false,
  gate_accepted: false,
})) {
  if (evidence.source_contract?.[key] !== expected) failures.push(`evidence source_contract.${key} must be ${expected}`);
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) if (value !== false) failures.push(`evidence validation.${key} must remain false`);

if (gate.accepted !== false) failures.push("Pages reference-consumer gate must remain unaccepted");
if (gate.current_boundary?.provider_health !== "unobserved") failures.push("provider health must remain unobserved");
if (gate.current_boundary?.four_profile_runtime_matrix !== "harness_source_ready_maintainer_execution_pending") failures.push("runtime matrix harness must remain maintainer-execution pending");

for (const marker of [
  "pages-server-owner-graphql-source-ready",
  "stateless-admin-transport-source-ready",
  "browser-intent-preflight-binding-source-ready",
  "No tests, Node verifiers, Cargo commands",
]) need(sources.actualization, marker, "actualization");

if (failures.length) {
  console.error("[verify-pages-builder-rollout-server-snapshot] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-builder-rollout-server-snapshot] PASS owner=pages_graphql admin=stateless bindings=ui+ssr+browser_intent runtime_matrix_harness=maintainer_execution_pending");
