#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
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
  health: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-consumer-binding-source.json",
  gate: "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json",
  actualization: "docs/modules/pages-page-builder-rollout-server-snapshot-actualization-2026-08-08.md",
};
const read = (name) => {
  const file = path.join(root, files[name]);
  if (!fs.existsSync(file) || !fs.lstatSync(file).isFile() || fs.lstatSync(file).isSymbolicLink()) {
    failures.push(`${name}: missing regular source file`);
    return "";
  }
  return fs.readFileSync(file, "utf8");
};
const source = Object.fromEntries(Object.keys(files).map((name) => [name, read(name)]));
const need = (text, marker, label) => { if (!text.includes(marker)) failures.push(`${label}: missing ${marker}`); };
const forbid = (text, marker, label) => { if (text.includes(marker)) failures.push(`${label}: forbidden ${marker}`); };
const evidence = JSON.parse(source.evidence || "{}");
const health = JSON.parse(source.health || "{}");
const gate = JSON.parse(source.gate || "{}");

for (const marker of [
  "pub fn from_module_settings(settings: &Value)",
  "module_settings_normalization_matches_all_declared_profiles",
  "module_settings_normalization_defaults_missing_keys_but_rejects_bad_types",
]) need(source.rollout, marker, "shared rollout normalizer");
for (const marker of [
  "page_builder_rollout_snapshot",
  "require_module_enabled(ctx, MODULE_SLUG).await?",
  "auth.tenant_id != tenant.id",
  "Permission::PAGES_READ",
  "tenant_module_settings(db, tenant.id, MODULE_SLUG)",
  "BuilderCapabilityFlags::from_module_settings(&settings)",
  "provider_health_observed: false",
  "provider_health: None",
  "PagesGraphqlRuntimeData::provider_health_snapshot",
]) need(source.owner, marker, "Pages server rollout owner");

for (const marker of [
  "fetch_pages_builder_rollout_snapshot(",
  "PagesBuilderRolloutSnapshotError::TenantMismatch",
  "pub fn provider_status(&self) -> PageBuilderAdminProviderStatus",
  "pub fn effective_runtime_flags(&self) -> BuilderCapabilityFlags",
  "pages_editor_capabilities_for_snapshot(",
]) need(source.adapter, marker, "stateless admin rollout/health adapter");
for (const marker of ["HostRuntimeContext", "tenant_module_settings(", "leptos_axum::extract"])
  forbid(source.adapter, marker, "admin rollout adapter");

for (const marker of [
  "pageBuilderRolloutSnapshot",
  "providerHealthObserved providerHealth",
  "payload.provider_health_observed",
  "ProviderHealthSnapshot::evaluate",
]) need(source.transport, marker, "GraphQL rollout/health transport");

for (const marker of [
  "fetch_pages_builder_rollout_snapshot(",
  "trusted_rollout.effective_runtime_flags()",
  "compose_fly_page_builder_handlers(store, renderer, effective_flags)",
]) need(source.builder, marker, "authoritative SSR binding");
for (const marker of ["fetch_pages_builder_rollout_snapshot(", ".provider_status();", ".with_provider_status(provider_status)"])
  need(source.composition, marker, "workspace binding");
for (const marker of [
  "fetch_pages_builder_rollout_snapshot(",
  "pages_editor_capabilities_for_snapshot(",
  "dispatch_pages_browser_intent_with_capabilities(snapshot, envelope, editor_capabilities)",
]) need(source.adminMain, marker, "standalone browser-intent binding");

forbid(source.cargo, "dep:leptos_axum", "Pages admin rollout dependencies");
forbid(source.cargo, "rustok-api/server", "Pages admin rollout dependencies");

if (evidence.format !== "pages_builder_rollout_server_snapshot_source_v1") failures.push("evidence format drifted");
if (evidence.status !== "server_owner_graphql_snapshot_and_health_aware_admin_bindings_source_ready_runtime_evidence_pending") failures.push("evidence status drifted");
for (const [key, expected] of Object.entries({
  tenant_context_is_server_resolved: true,
  auth_context_is_server_resolved: true,
  auth_tenant_must_match_routed_tenant: true,
  pages_read_authority_required: true,
  raw_settings_are_not_returned: true,
  typed_optional_provider_health_returned: true,
  provider_health_default_observed_value: false,
  fresh_accepted_server_authority_can_supply_observed_health: true,
  pages_ui_provider_health_source_binding_complete: true,
  pages_ssr_provider_health_source_binding_complete: true,
  standalone_browser_intent_provider_health_source_binding_complete: true,
  provider_health_only_narrows_rollout_and_role_capabilities: true,
  four_profile_runtime_evidence_retained: false,
  observed_provider_health_runtime_evidence_retained: false,
  gate_accepted: false,
})) if (evidence.source_contract?.[key] !== expected) failures.push(`source_contract.${key} must be ${expected}`);
for (const [key, value] of Object.entries(evidence.validation ?? {})) if (value !== false) failures.push(`validation.${key} must remain false`);
if (health.format !== "pages_builder_provider_health_consumer_binding_source_v1") failures.push("health consumer continuation missing");
if (gate.accepted !== false) failures.push("Pages reference-consumer gate must remain unaccepted");
if (gate.current_boundary?.provider_health !== "unobserved") failures.push("retained gate provider-health execution boundary must remain unobserved");
if (gate.current_boundary?.four_profile_runtime_matrix !== "harness_source_ready_maintainer_execution_pending") failures.push("runtime matrix must remain maintainer-execution pending");

for (const marker of [
  "pages-server-owner-graphql-source-ready",
  "stateless-admin-transport-source-ready",
  "browser-intent-preflight-binding-source-ready",
  "No tests, Node verifiers, Cargo commands",
]) need(source.actualization, marker, "rollout server snapshot historical actualization");

if (failures.length) {
  console.error("[verify-pages-builder-rollout-server-snapshot] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-builder-rollout-server-snapshot] PASS owner=server_graphql health_transport=typed bindings=ui+ssr+browser_intent runtime_evidence=pending");
