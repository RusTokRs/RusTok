#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const files = {
  builder: "crates/rustok-pages/admin/src/builder.rs",
  adapter: "crates/rustok-pages/admin/src/builder_rollout_settings.rs",
  composition: "crates/rustok-pages/admin/src/composition.rs",
  evidence: "crates/rustok-pages/contracts/evidence/pages-builder-rollout-binding-source.json",
  gate: "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json",
  actualization: "docs/modules/pages-page-builder-rollout-binding-actualization-2026-08-08.md",
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
  console.error("[verify-pages-builder-rollout-binding] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const sources = Object.fromEntries(Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]));
const evidence = JSON.parse(sources.evidence);
const gate = JSON.parse(sources.gate);

for (const marker of [
  "provider_flags: Option<BuilderCapabilityFlags>",
  "with_provider_flags",
  ".clone()",
  ".map(PageBuilderAdminProviderStatus::unobserved)",
  "load_trusted_pages_builder_rollout_snapshot()",
  "tenant_slug != trusted_rollout.tenant_slug",
  "compose_fly_page_builder_handlers(store, renderer, trusted_rollout.flags)",
]) need(sources.builder, marker, "builder");
for (const marker of [
  "fn pages_builder_capability_flags() -> BuilderCapabilityFlags",
  "compose_fly_page_builder_handlers(store, renderer, pages_builder_capability_flags())",
]) forbid(sources.builder, marker, "builder hardcoded all-on path");

for (const marker of [
  "TrustedPagesBuilderRolloutSnapshot",
  "load_trusted_pages_builder_rollout_snapshot(",
  "auth.tenant_id != tenant.id",
  'tenant_module_settings(runtime.db(), tenant.id, "pages")',
  "tenant_slug: tenant.slug",
]) need(sources.adapter, marker, "trusted adapter");

for (const marker of [
  "pages_builder_rollout_flags()",
  "provider_flags: BuilderCapabilityFlags",
  ".with_provider_flags(provider_flags)",
]) need(sources.composition, marker, "composition");

if (evidence.format !== "pages_builder_rollout_binding_source_v1") failures.push(`evidence format drifted: ${evidence.format}`);
if (evidence.status !== "ui_and_authoritative_ssr_binding_source_ready_runtime_matrix_pending") failures.push(`evidence status drifted: ${evidence.status}`);
for (const [key, expected] of Object.entries({
  ui_workspace_loads_server_owned_flags: true,
  ui_facade_provider_status_uses_loaded_flags: true,
  ui_facade_provider_health_remains_unobserved: true,
  authoritative_ssr_rereads_trusted_snapshot_per_request: true,
  authoritative_ssr_verifies_snapshot_tenant_slug: true,
  authoritative_ssr_composes_handlers_from_trusted_flags: true,
  browser_rollout_flags_are_authoritative: false,
  hardcoded_all_on_consumer_binding_present: false,
  four_profiles_source_exercisable: true,
  four_profile_runtime_evidence_retained: false,
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
if (gate.current_boundary?.trusted_rollout_server_snapshot !== "source_ready_ui_and_ssr_binding_complete") failures.push("trusted rollout binding cursor drifted");
if (gate.current_boundary?.four_profile_runtime_matrix !== "source_executable_evidence_pending") failures.push("four-profile runtime cursor drifted");
if (gate.current_boundary?.provider_health !== "unobserved") failures.push("provider health must remain unobserved");

for (const marker of [
  "ui-provider-binding-source-ready",
  "authoritative-ssr-binding-source-ready",
  "runtime-matrix-evidence-pending",
  "No tests, Node verifiers, Cargo commands",
]) need(sources.actualization, marker, "actualization");

if (failures.length > 0) {
  console.error("[verify-pages-builder-rollout-binding] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-builder-rollout-binding] PASS ui_binding=source_ready ssr_binding=source_ready runtime_matrix=evidence_pending");
