#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const files = {
  adapter: "crates/rustok-pages/admin/src/builder_rollout_settings.rs",
  builder: "crates/rustok-pages/admin/src/builder.rs",
  composition: "crates/rustok-pages/admin/src/composition.rs",
  evidence: "crates/rustok-pages/contracts/evidence/pages-builder-rollout-server-snapshot-source.json",
  gate: "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json",
  actualization: "docs/modules/pages-page-builder-rollout-server-snapshot-actualization-2026-08-08.md",
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
  if (!stats.isFile() || stats.isSymbolicLink()) failures.push(`${label}: ${relativePath} must be a regular non-symlink file`);
}
if (failures.length > 0) {
  console.error("[verify-pages-builder-rollout-server-snapshot] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const sources = Object.fromEntries(Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]));
const evidence = JSON.parse(sources.evidence);
const gate = JSON.parse(sources.gate);

for (const marker of [
  "pages_builder_flags_from_settings(",
  "current.as_bool().ok_or_else",
  "flags.validate()?",
  "ensure_trusted_tenant(",
  "auth.tenant_id != tenant.id",
  "Permission::new(Resource::Pages, Action::Read)",
  "TrustedPagesBuilderRolloutSnapshot",
  "load_trusted_pages_builder_rollout_snapshot(",
  'tenant_module_settings(runtime.db(), tenant.id, "pages")',
  "tenant_slug: tenant.slug",
  '#[server(prefix = "/api/fn", endpoint = "pages/builder-rollout-flags")]',
  "Ok(load_trusted_pages_builder_rollout_snapshot().await?.flags)",
  "declared_profiles_normalize_to_their_exact_flags",
  "malformed_setting_types_fail_closed",
  "invalid_flag_combinations_fail_closed",
]) need(sources.adapter, marker, "trusted rollout server adapter");
for (const marker of ["localStorage", "sessionStorage", "query parameter", "browser_supplied_flags"]) {
  forbid(sources.adapter, marker, "trusted rollout adapter");
}

for (const marker of [
  "provider_flags: Option<BuilderCapabilityFlags>",
  "provider_flags: None",
  "with_provider_flags",
  ".map(PageBuilderAdminProviderStatus::unobserved)",
  "load_trusted_pages_builder_rollout_snapshot()",
  "tenant_slug != trusted_rollout.tenant_slug",
  "compose_fly_page_builder_handlers(store, renderer, trusted_rollout.flags)",
]) need(sources.builder, marker, "Pages builder rollout bindings");
for (const marker of [
  "fn pages_builder_capability_flags() -> BuilderCapabilityFlags",
  "compose_fly_page_builder_handlers(store, renderer, pages_builder_capability_flags())",
]) forbid(sources.builder, marker, "hardcoded all-on consumer binding");

for (const marker of [
  "pages_builder_rollout_flags()",
  "provider_flags: BuilderCapabilityFlags",
  ".with_provider_flags(provider_flags)",
]) need(sources.composition, marker, "Pages workspace provider binding");

if (evidence.format !== "pages_builder_rollout_server_snapshot_source_v1") failures.push(`evidence format drifted: ${evidence.format}`);
if (evidence.status !== "trusted_server_snapshot_source_ready_ui_and_ssr_binding_complete_runtime_evidence_pending") {
  failures.push(`evidence status drifted: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  tenant_context_is_server_extracted: true,
  auth_context_is_server_extracted: true,
  auth_tenant_must_match_routed_tenant: true,
  pages_read_authority_required: true,
  module_slug_is_fixed_to_pages: true,
  enabled_pages_row_required: true,
  raw_settings_are_not_returned: true,
  only_builder_capability_flags_are_returned: true,
  omitted_builder_keys_default_to_all_on_for_backward_compatibility: true,
  malformed_setting_types_fail_closed: true,
  invalid_flag_combinations_fail_closed: true,
  all_four_declared_profiles_have_source_tests: true,
  browser_supplied_flags_accepted: false,
  settings_mutation_added: false,
  pages_ui_facade_binding_complete: true,
  pages_ui_facade_uses_server_owned_flags: true,
  pages_ssr_dispatch_binding_complete: true,
  pages_ssr_dispatch_rereads_trusted_snapshot_per_request: true,
  pages_ssr_snapshot_tenant_slug_must_match_request_snapshot: true,
  hardcoded_all_on_consumer_binding_removed: true,
  four_profile_runtime_matrix_executable: true,
  four_profile_runtime_evidence_retained: false,
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
if (gate.current_boundary?.trusted_rollout_server_snapshot !== "source_ready_ui_and_ssr_binding_complete") {
  failures.push("Pages gate trusted rollout snapshot cursor drifted");
}
if (gate.current_boundary?.four_profile_runtime_matrix !== "source_executable_evidence_pending") {
  failures.push("Pages gate runtime matrix cursor drifted");
}
if (gate.current_boundary?.provider_health !== "unobserved") failures.push("provider health must remain unobserved");

for (const marker of [
  "UI facade binding",
  "Authoritative SSR dispatch binding",
  "freshly reread trusted flags",
  "four-profile runtime-evidence-pending",
  "No tests, Node verifiers, Cargo commands",
]) need(sources.actualization, marker, "actualization");

if (failures.length > 0) {
  console.error("[verify-pages-builder-rollout-server-snapshot] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-builder-rollout-server-snapshot] PASS trusted_snapshot=source_ready ui_binding=source_ready ssr_binding=source_ready runtime_evidence=pending");
