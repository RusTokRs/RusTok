#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const files = {
  runtime: "crates/rustok-api/src/runtime.rs",
  rollout: "crates/rustok-page-builder/src/rollout.rs",
  owner: "crates/rustok-pages/src/graphql/builder_rollout.rs",
  adapter: "crates/rustok-pages/admin/src/builder_rollout_settings.rs",
  builder: "crates/rustok-pages/admin/src/builder.rs",
  composition: "crates/rustok-pages/admin/src/composition.rs",
  adminMain: "apps/admin/src/main.rs",
  evidence: "crates/rustok-pages/contracts/evidence/pages-tenant-rollout-settings-runtime-source.json",
  gate: "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json",
  actualization: "docs/modules/pages-page-builder-tenant-rollout-settings-runtime-actualization-2026-08-08.md",
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
  console.error("[verify-pages-tenant-rollout-settings-runtime] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const sources = Object.fromEntries(Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]));
const evidence = JSON.parse(sources.evidence);
const gate = JSON.parse(sources.gate);

for (const marker of [
  "pub async fn tenant_module_settings(",
  "tenant id from a trusted request context",
  "enabled = 1",
  "settings::text AS settings_json",
  "CAST(settings AS CHAR) AS settings_json",
  "serde_json::from_str(&encoded)",
]) need(sources.runtime, marker, "platform settings read seam");
for (const marker of ["INSERT ", "UPDATE ", "DELETE ", "ActiveModel"]) {
  const start = sources.runtime.indexOf("pub async fn tenant_module_settings(");
  const end = sources.runtime.indexOf("\n/// Immutable host configuration snapshot", start);
  const helper = start >= 0 && end > start ? sources.runtime.slice(start, end) : "";
  forbid(helper, marker, "platform settings helper must remain read-only");
}

for (const marker of [
  "pub fn from_module_settings(settings: &Value)",
  "flags.validate()?",
]) need(sources.rollout, marker, "shared rollout normalizer");
for (const marker of [
  "page_builder_rollout_snapshot",
  "auth.tenant_id != tenant.id",
  "Permission::PAGES_READ",
  "tenant_module_settings(db, tenant.id, MODULE_SLUG)",
  "BuilderCapabilityFlags::from_module_settings(&settings)",
]) need(sources.owner, marker, "Pages server rollout owner");

for (const marker of [
  "fetch_pages_builder_rollout_snapshot(",
  "PagesBuilderRolloutSnapshotError::TenantMismatch",
  "pages_editor_capabilities_for_rollout(",
]) need(sources.adapter, marker, "stateless admin rollout adapter");
for (const marker of ["HostRuntimeContext", "tenant_module_settings(", "leptos_axum::extract"])
  forbid(sources.adapter, marker, "admin rollout adapter must stay stateless");

need(sources.builder, "fetch_pages_builder_rollout_snapshot(", "SSR rollout binding");
need(sources.builder, "compose_fly_page_builder_handlers(store, renderer, trusted_rollout.flags)", "SSR rollout binding");
need(sources.composition, "fetch_pages_builder_rollout_snapshot(", "workspace rollout binding");
for (const marker of [
  "fetch_pages_builder_rollout_snapshot(",
  "pages_editor_capabilities_for_rollout(",
  "dispatch_pages_browser_intent_with_capabilities(snapshot, envelope, editor_capabilities)",
]) need(sources.adminMain, marker, "browser-intent rollout preflight");

if (evidence.format !== "pages_tenant_rollout_settings_runtime_source_v1") failures.push(`evidence format drifted: ${evidence.format}`);
if (evidence.status !== "platform_read_seam_server_owner_graphql_and_all_pages_bindings_source_ready_runtime_evidence_pending") failures.push(`evidence status drifted: ${evidence.status}`);
for (const [key, expected] of Object.entries({
  trusted_tenant_id_must_be_supplied_by_server_owner: true,
  settings_mutation_added: false,
  parallel_settings_store_added: false,
  server_owner_graphql_snapshot_complete: true,
  shared_rollout_normalizer_complete: true,
  pages_consumer_binding_complete: true,
  pages_ui_provider_status_uses_server_owned_flags: true,
  pages_ssr_capability_dispatch_rereads_server_owned_flags: true,
  standalone_browser_intent_preflight_uses_server_owned_flags: true,
  browser_supplied_flags_accepted: false,
  pages_runtime_profile_matrix_evidence_retained: false,
  provider_health_observed: false,
  gate_accepted: false,
})) {
  if (evidence.source_contract?.[key] !== expected) failures.push(`evidence source_contract.${key} must be ${expected}`);
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) if (value !== false) failures.push(`evidence validation.${key} must remain false`);

if (gate.accepted !== false) failures.push("Pages reference-consumer gate must remain unaccepted");
if (gate.current_boundary?.four_profile_runtime_matrix !== "harness_source_ready_maintainer_execution_pending") failures.push("four-profile runtime matrix harness must remain maintainer-execution pending");
if (gate.current_boundary?.provider_health !== "unobserved") failures.push("provider health must remain unobserved");

for (const marker of [
  "pages-server-owner-graphql-source-ready",
  "all-consumer-bindings-source-ready",
  "four-profile-runtime-evidence-pending",
  "No tests, Node verifiers, Cargo commands",
]) need(sources.actualization, marker, "actualization");

if (failures.length) {
  console.error("[verify-pages-tenant-rollout-settings-runtime] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-tenant-rollout-settings-runtime] PASS owner=pages_graphql bindings=ui+ssr+browser_intent runtime_matrix_harness=maintainer_execution_pending");
