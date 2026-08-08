#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const files = {
  adapter: "crates/rustok-pages/admin/src/builder_rollout_settings.rs",
  cargo: "crates/rustok-pages/admin/Cargo.toml",
  lib: "crates/rustok-pages/admin/src/lib.rs",
  builder: "crates/rustok-pages/admin/src/builder.rs",
  runtime: "crates/rustok-api/src/runtime.rs",
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
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${label}: ${relativePath} must be a regular non-symlink file`);
  }
}
if (failures.length > 0) {
  console.error("[verify-pages-builder-rollout-server-snapshot] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);
const gate = JSON.parse(sources.gate);

for (const marker of [
  "pages_builder_flags_from_settings(",
  'nested_bool(settings, &["builder", "enabled"])',
  'nested_bool(settings, &["builder", "preview", "enabled"])',
  'nested_bool(settings, &["builder", "properties", "enabled"])',
  'nested_bool(settings, &["builder", "publish", "enabled"])',
  "current.as_bool().ok_or_else",
  "flags.validate()?",
  "ensure_trusted_tenant(",
  "auth.tenant_id != tenant.id",
  "Permission::new(Resource::Pages, Action::Read)",
  '#[server(prefix = "/api/fn", endpoint = "pages/builder-rollout-flags")]',
  "expect_context::<HostRuntimeContext>()",
  "leptos_axum::extract::<AuthContext>()",
  "leptos_axum::extract::<TenantContext>()",
  'tenant_module_settings(runtime.db(), tenant.id, "pages")',
  "Pages module is not enabled for the routed tenant",
  "Pages builder rollout settings are invalid",
  "declared_profiles_normalize_to_their_exact_flags",
  "BuilderToggleProfile::ALL",
  "omitted_builder_settings_preserve_backward_compatible_all_on_defaults",
  "malformed_setting_types_fail_closed",
  "invalid_flag_combinations_fail_closed",
]) need(sources.adapter, marker, "trusted rollout server adapter");

for (const marker of [
  "browser",
  "query",
  "header",
  "localStorage",
  "sessionStorage",
]) forbid(sources.adapter, marker, "rollout adapter must not accept browser-controlled flags");

for (const marker of [
  '"rustok-api/server"',
  '"dep:leptos_axum"',
  "leptos_axum = { workspace = true, optional = true }",
]) need(sources.cargo, marker, "Pages admin SSR dependencies");
need(sources.lib, "mod builder_rollout_settings;", "Pages admin module registration");
need(sources.runtime, "pub async fn tenant_module_settings(", "platform settings read seam");

for (const marker of [
  "fn pages_builder_capability_flags() -> BuilderCapabilityFlags",
  "BuilderCapabilityFlags::default()",
  "compose_fly_page_builder_handlers(store, renderer, pages_builder_capability_flags())",
]) need(sources.builder, marker, "remaining Pages UI/dispatch binding blocker");

if (evidence.format !== "pages_builder_rollout_server_snapshot_source_v1") {
  failures.push(`evidence format drifted: ${evidence.format}`);
}
if (evidence.status !== "trusted_server_snapshot_source_ready_ui_dispatch_binding_pending") {
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
  pages_ui_facade_binding_complete: false,
  pages_ssr_dispatch_binding_complete: false,
  four_profile_runtime_matrix_executable: false,
  gate_accepted: false,
  forum_wave_accepted: false,
  ffa_promoted: false,
  fba_promoted: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence source_contract.${key} must be ${expected}`);
  }
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`evidence validation.${key} must remain false`);
}

if (gate.accepted !== false) failures.push("Pages reference-consumer gate must remain unaccepted");
if (
  gate.current_boundary?.trusted_rollout_server_snapshot !==
  "source_ready_ui_and_dispatch_binding_pending"
) {
  failures.push("Pages gate trusted rollout server snapshot cursor drifted");
}
if (
  gate.verification?.trusted_rollout_server_snapshot_guard !==
  "crates/rustok-pages/scripts/verify/verify-pages-builder-rollout-server-snapshot.mjs"
) {
  failures.push("Pages gate trusted rollout server snapshot guard is not registered");
}

for (const marker of [
  "trusted server snapshot is source-ready",
  "UI facade binding remains pending",
  "SSR capability dispatch binding remains pending",
  "browser never supplies rollout flags",
  "No tests, Node verifiers, Cargo commands",
]) need(sources.actualization, marker, "actualization");

if (failures.length > 0) {
  console.error("[verify-pages-builder-rollout-server-snapshot] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "[verify-pages-builder-rollout-server-snapshot] PASS trusted_server_snapshot=source_ready ui_binding=pending ssr_dispatch_binding=pending",
);
