#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const files = {
  runtime: "crates/rustok-api/src/runtime.rs",
  apiLib: "crates/rustok-api/src/lib.rs",
  adapter: "crates/rustok-pages/admin/src/builder_rollout_settings.rs",
  pagesBuilder: "crates/rustok-pages/admin/src/builder.rs",
  composition: "crates/rustok-pages/admin/src/composition.rs",
  evidence: "crates/rustok-pages/contracts/evidence/pages-tenant-rollout-settings-runtime-source.json",
  gate: "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json",
  actualization: "docs/modules/pages-page-builder-tenant-rollout-settings-runtime-actualization-2026-08-08.md",
};
const failures = [];
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
  console.error("[verify-pages-tenant-rollout-settings-runtime] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);
const gate = JSON.parse(sources.gate);

const functionStart = sources.runtime.indexOf("pub async fn tenant_module_settings(");
const functionEnd = sources.runtime.indexOf("\n/// Immutable host configuration snapshot", functionStart);
if (functionStart < 0 || functionEnd < 0) failures.push("runtime: unable to isolate tenant_module_settings");
const settingsFunction = functionStart >= 0 && functionEnd > functionStart
  ? sources.runtime.slice(functionStart, functionEnd)
  : "";

for (const marker of [
  "pub async fn tenant_module_settings(",
  "tenant_id: Uuid",
  "module_slug: &str",
  "caller must supply the",
  "tenant id from a trusted request context",
  "enabled = 1",
  "enabled = true",
  "CAST(settings AS TEXT) AS settings_json",
  "settings::text AS settings_json",
  "CAST(settings AS CHAR) AS settings_json",
  'row.try_get("", "settings_json")?',
  "serde_json::from_str(&encoded)",
  "DbErr::Custom",
]) need(settingsFunction, marker, "runtime settings helper");
for (const marker of ["INSERT ", "UPDATE ", "DELETE ", "ActiveModel", ".execute(", "execute_unprepared("]) {
  forbid(settingsFunction, marker, "runtime settings helper must remain read-only");
}
need(sources.apiLib, "tenant_module_settings", "rustok-api runtime export");

for (const marker of [
  "TrustedPagesBuilderRolloutSnapshot",
  "load_trusted_pages_builder_rollout_snapshot(",
  "auth.tenant_id != tenant.id",
  "Permission::new(Resource::Pages, Action::Read)",
  'tenant_module_settings(runtime.db(), tenant.id, "pages")',
  "tenant_slug: tenant.slug",
  "pages_builder_flags_from_settings(&settings)",
]) need(sources.adapter, marker, "trusted Pages rollout adapter");

for (const marker of [
  "provider_flags: Option<BuilderCapabilityFlags>",
  "with_provider_flags",
  ".map(PageBuilderAdminProviderStatus::unobserved)",
  "load_trusted_pages_builder_rollout_snapshot()",
  "tenant_slug != trusted_rollout.tenant_slug",
  "compose_fly_page_builder_handlers(store, renderer, trusted_rollout.flags)",
]) need(sources.pagesBuilder, marker, "Pages builder binding");
for (const marker of [
  "fn pages_builder_capability_flags() -> BuilderCapabilityFlags",
  "compose_fly_page_builder_handlers(store, renderer, pages_builder_capability_flags())",
]) forbid(sources.pagesBuilder, marker, "hardcoded Pages rollout binding");

for (const marker of [
  "pages_builder_rollout_flags()",
  ".with_provider_flags(provider_flags)",
  "provider_flags: BuilderCapabilityFlags",
]) need(sources.composition, marker, "Pages workspace rollout binding");

if (evidence.format !== "pages_tenant_rollout_settings_runtime_source_v1") failures.push(`evidence format drifted: ${evidence.format}`);
if (evidence.status !== "platform_settings_read_seam_and_pages_binding_source_ready_runtime_evidence_pending") {
  failures.push(`evidence status drifted: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  trusted_tenant_id_must_be_supplied_by_caller: true,
  exact_tenant_id_required: true,
  exact_module_slug_required: true,
  enabled_row_required: true,
  disabled_row_returns_none: true,
  missing_row_returns_none: true,
  settings_json_decode_is_fail_closed: true,
  sqlite_supported: true,
  postgres_supported: true,
  mysql_supported: true,
  tenant_persistence_entity_import_required: false,
  settings_mutation_added: false,
  parallel_settings_store_added: false,
  pages_consumer_binding_complete: true,
  pages_ui_provider_status_uses_trusted_flags: true,
  pages_ssr_capability_dispatch_rereads_trusted_flags: true,
  pages_ssr_snapshot_tenant_slug_is_verified: true,
  browser_supplied_flags_accepted: false,
  pages_runtime_profile_matrix_executable: true,
  pages_runtime_profile_matrix_evidence_retained: false,
  provider_health_observed: false,
  gate_accepted: false,
  forum_wave_accepted: false,
  ffa_promoted: false,
  fba_promoted: false,
})) {
  if (evidence.source_contract?.[key] !== expected) failures.push(`evidence source_contract.${key} must be ${expected}`);
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`evidence validation.${key} must remain false`);
}

if (gate.accepted !== false) failures.push("Pages reference-consumer gate must remain unaccepted");
if (gate.current_boundary?.execution_gate !== "pending") failures.push("Pages execution gate must remain pending");
if (gate.current_boundary?.tenant_rollout_settings !== "platform_read_seam_and_pages_binding_source_ready") {
  failures.push("Pages gate tenant rollout settings cursor drifted");
}
if (gate.current_boundary?.four_profile_runtime_matrix !== "source_executable_evidence_pending") {
  failures.push("Pages gate four-profile runtime cursor drifted");
}
if (gate.current_boundary?.provider_health !== "unobserved") failures.push("provider health must remain unobserved");

for (const marker of [
  "Pages consumer binding is source-ready",
  "independently calls `load_trusted_pages_builder_rollout_snapshot()`",
  "four-profile runtime evidence-pending",
  "No tests, Node verifiers, Cargo commands",
]) need(sources.actualization, marker, "actualization");

if (failures.length > 0) {
  console.error("[verify-pages-tenant-rollout-settings-runtime] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-tenant-rollout-settings-runtime] PASS platform_read_seam=source_ready pages_binding=source_ready four_profile_runtime=evidence_pending");
