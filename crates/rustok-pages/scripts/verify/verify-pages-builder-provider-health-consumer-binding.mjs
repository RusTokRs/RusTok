#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  contract: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-consumer-binding-source.json",
  coreRollout: "crates/rustok-page-builder/src/rollout.rs",
  providerStatus: "crates/rustok-page-builder/admin/src/provider_status.rs",
  owner: "crates/rustok-pages/src/graphql/builder_rollout.rs",
  rollout: "crates/rustok-pages/admin/src/builder_rollout_settings.rs",
  facade: "crates/rustok-pages/admin/src/builder.rs",
  composition: "crates/rustok-pages/admin/src/composition.rs",
  adminMain: "apps/admin/src/main.rs",
  serverBinding: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-server-binding-source.json",
  transport: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-transport-source.json",
  preflightEvidence: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-capability-preflight-source.json",
  runtimeHarness: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-runtime-harness-source.json",
  gate: "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json",
  overlay: "docs/modules/pages-page-builder-provider-health-consumer-binding-actualization-2026-08-09.md",
  parity: "docs/modules/pages-page-builder-plan-parity-actualization-2026-08-08.md",
};
const abs = (value) => path.join(repoRoot, value);
const read = (value) => fs.readFileSync(abs(value), "utf8");
const need = (source, marker, label) => { if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`); };
const forbid = (source, marker, label) => { if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`); };

for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(abs(relativePath))) { failures.push(`${label}: missing ${relativePath}`); continue; }
  const stats = fs.lstatSync(abs(relativePath));
  if (!stats.isFile() || stats.isSymbolicLink()) failures.push(`${label}: ${relativePath} must be a regular non-symlink file`);
}
if (failures.length) {
  console.error("[verify-pages-builder-provider-health-consumer-binding] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const sources = Object.fromEntries(Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]));
const contract = JSON.parse(sources.contract);
const serverBinding = JSON.parse(sources.serverBinding);
const transport = JSON.parse(sources.transport);
const preflightEvidence = JSON.parse(sources.preflightEvidence);
const runtimeHarness = JSON.parse(sources.runtimeHarness);
const gate = JSON.parse(sources.gate);

if (contract.format !== "pages_builder_provider_health_consumer_binding_source_v1" || contract.status !== "source_ready_runtime_activation_pending") failures.push("consumer binding source identity drifted");
if (serverBinding.format !== "pages_builder_provider_health_server_binding_source_v1") failures.push("server binding predecessor drifted");
if (transport.format !== "pages_builder_provider_health_transport_source_v1") failures.push("transport predecessor drifted");
if (preflightEvidence.format !== "pages_builder_provider_health_capability_preflight_source_v1") failures.push("health-aware capability preflight predecessor drifted");
if (runtimeHarness.format !== "pages_builder_provider_health_runtime_harness_source_v1" || runtimeHarness.status !== "source_ready_maintainer_execution_pending") failures.push("runtime evidence harness continuation is not source-ready");

for (const [object, key, expected] of [
  [contract.canonical_provider_status, "configured_rollout_flags_remain_authoritative", true],
  [contract.canonical_provider_status, "observed_health_may_only_narrow", true],
  [contract.canonical_provider_status, "unobserved_preserves_configured_rollout", true],
  [contract.canonical_provider_status, "degraded_disables_publish", true],
  [contract.canonical_provider_status, "unavailable_disables_builder", true],
  [contract.canonical_provider_status, "runtime_flags_shared_policy", "rustok_page_builder::rollout::effective_provider_runtime_flags"],
  [contract.workspace_binding, "uses_validated_provider_status", true],
  [contract.workspace_binding, "facade_receives_provider_status", true],
  [contract.authoritative_ssr_binding, "rereads_server_owned_snapshot_per_capability_request", true],
  [contract.authoritative_ssr_binding, "uses_effective_runtime_flags", true],
  [contract.authoritative_ssr_binding, "missing_health_uses_configured_rollout_flags", true],
  [contract.standalone_browser_intent_binding, "uses_pages_editor_capabilities_for_snapshot", true],
  [contract.standalone_browser_intent_binding, "role_capabilities_evaluated_before_provider_narrowing", true],
  [contract.non_mutating_capability_preflight, "operation", "pageBuilderCapabilityPreflight"],
  [contract.non_mutating_capability_preflight, "uses_fresh_provider_health_authority", true],
  [contract.non_mutating_capability_preflight, "uses_shared_runtime_flags_policy", true],
  [contract.non_mutating_capability_preflight, "uses_canonical_feature_disabled_guard", true],
  [contract.anti_promotion, "accepted_packet_installed", false],
  [contract.anti_promotion, "observed_capability_preflight_executed", false],
  [contract.anti_promotion, "health_driven_workspace_behavior_executed", false],
  [contract.anti_promotion, "health_driven_ssr_behavior_executed", false],
  [contract.anti_promotion, "health_driven_browser_intent_behavior_executed", false],
  [contract.anti_promotion, "pages_reference_consumer_gate_accepted", false],
  [contract.validation, "tests_run", false],
  [contract.validation, "node_verifier_run", false],
]) if (object?.[key] !== expected) failures.push(`${key} must equal ${JSON.stringify(expected)}`);

for (const marker of ["pub fn effective_provider_runtime_flags(", "effective.publish_enabled = false"])
  need(sources.coreRollout, marker, "shared provider runtime policy");
for (const marker of ["pub fn effective_runtime_flags(&self) -> BuilderCapabilityFlags", "effective_provider_runtime_flags(&self.flags, self.health.as_ref())"])
  need(sources.providerStatus, marker, "admin provider status delegation");
for (const marker of [
  "fn provider_health_snapshot(ctx: &Context<'_>) -> Option<ProviderHealthSnapshot>",
  "let provider_health = provider_health_snapshot(ctx);",
  "effective_provider_runtime_flags(&flags, provider_health.as_ref())",
  "ensure_capability(&effective_flags, capability_kind)",
]) need(sources.owner, marker, "health-aware GraphQL preflight");
for (const forbidden of ["std::fs::", "RUSTOK_PAGES_PROVIDER_HEALTH_ACCEPTANCE_PATH"]) forbid(sources.owner, forbidden, "GraphQL must consume typed runtime authority");

for (const marker of [
  "pub fn provider_status(&self) -> PageBuilderAdminProviderStatus",
  "self.provider_status().effective_runtime_flags()",
  "pages_editor_capabilities_for_snapshot(",
  "snapshot.provider_status().limit_capabilities(capabilities)",
]) need(sources.rollout, marker, "Pages snapshot/provider status seam");
for (const marker of [
  "provider_status: Option<PageBuilderAdminProviderStatus>",
  "pub fn with_provider_status(",
  "fetch_pages_builder_rollout_snapshot(",
  "let effective_flags = trusted_rollout.effective_runtime_flags();",
  "compose_fly_page_builder_handlers(store, renderer, effective_flags)",
]) need(sources.facade, marker, "Pages facade/SSR binding");
forbid(sources.facade, "compose_fly_page_builder_handlers(store, renderer, trusted_rollout.flags)", "SSR must not bypass health-limited flags");
for (const marker of [".provider_status();", "provider_status: PageBuilderAdminProviderStatus", ".with_provider_status(provider_status)"])
  need(sources.composition, marker, "workspace provider binding");
forbid(sources.composition, ".with_provider_flags(provider_flags)", "workspace must not discard health");
for (const marker of [
  "pages_editor_capabilities_for_snapshot",
  "pages_editor_capabilities_for_snapshot(role_capabilities, &rollout)",
  "dispatch_pages_browser_intent_with_capabilities(snapshot, envelope, editor_capabilities)",
]) need(sources.adminMain, marker, "standalone browser intent health binding");
forbid(sources.adminMain, "pages_editor_capabilities_for_rollout(role_capabilities, &rollout.flags)", "browser intent must not discard health");

if (gate.accepted !== false || gate.current_boundary?.provider_health !== "unobserved") failures.push("retained Pages gate must remain unaccepted/provider-health-unobserved");
for (const marker of ["consumer-provider-health-binding-source-ready", "authoritative-ssr-health-guard-source-ready", "runtime activation remains maintainer-owned"])
  need(sources.overlay, marker, "consumer binding actualization");
for (const marker of [
  "provider-health-consumer-binding-source-ready",
  "provider-health-capability-preflight-source-ready",
  "provider-health-runtime-evidence-harness-source-ready",
  "observed-health runtime evidence harness [source-ready / maintainer execution pending]",
]) need(sources.parity, marker, "canonical parity");

if (contract.next_cursor?.provider_health_consumer_binding !== "source_ready_runtime_activation_pending") failures.push("consumer binding cursor drifted");
if (contract.next_cursor?.provider_health_capability_preflight !== "source_ready_runtime_execution_pending") failures.push("capability preflight cursor drifted");
if (contract.next_cursor?.observed_health_runtime_evidence_harness !== "source_ready_maintainer_execution_pending") failures.push("runtime evidence harness cursor must be source-ready/maintainer-execution-pending");
if (contract.next_cursor?.retained_identity_evaluator_owner_acceptance !== "maintainer_execution_pending") failures.push("live evidence execution must remain maintainer-owned");
if (contract.next_cursor?.observed_health_acceptance !== "pending") failures.push("observed health acceptance must remain pending");

if (failures.length) {
  console.error("[verify-pages-builder-provider-health-consumer-binding] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-builder-provider-health-consumer-binding] PASS consumers=workspace+ssr+browser_intent preflight=health_aware runtime_harness=source_ready execution=pending");
