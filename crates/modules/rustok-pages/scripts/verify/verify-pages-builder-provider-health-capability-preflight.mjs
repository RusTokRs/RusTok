#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  contract: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-capability-preflight-source.json",
  coreRollout: "crates/rustok-page-builder/src/rollout.rs",
  providerStatus: "crates/rustok-page-builder/admin/src/provider_status.rs",
  owner: "crates/rustok-pages/src/graphql/builder_rollout.rs",
  runtimeData: "crates/rustok-pages/src/graphql/runtime_data.rs",
  pagesSnapshot: "crates/rustok-pages/admin/src/builder_rollout_settings.rs",
  pagesFacade: "crates/rustok-pages/admin/src/builder.rs",
  rolloutHarnessContract: "crates/rustok-pages/contracts/evidence/pages-builder-rollout-feature-preflight-execution-contract.json",
  rolloutHarnessEvidence: "crates/rustok-pages/contracts/evidence/pages-builder-rollout-feature-preflight-harness-source.json",
  rolloutHarnessSpec: "apps/next-admin/tests/pages-builder-rollout-feature-preflight/feature-preflight.spec.ts",
  runtimeHarness: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-runtime-harness-source.json",
  consumerBinding: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-consumer-binding-source.json",
  serverBinding: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-server-binding-source.json",
  gate: "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json",
  overlay: "docs/modules/pages-page-builder-provider-health-capability-preflight-actualization-2026-08-09.md",
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
  console.error("[verify-pages-builder-provider-health-capability-preflight] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const sources = Object.fromEntries(Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]));
const contract = JSON.parse(sources.contract);
const rolloutContract = JSON.parse(sources.rolloutHarnessContract);
const rolloutEvidence = JSON.parse(sources.rolloutHarnessEvidence);
const runtimeHarness = JSON.parse(sources.runtimeHarness);
const consumerBinding = JSON.parse(sources.consumerBinding);
const serverBinding = JSON.parse(sources.serverBinding);
const gate = JSON.parse(sources.gate);

if (contract.format !== "pages_builder_provider_health_capability_preflight_source_v1" || contract.status !== "source_ready_runtime_execution_pending") failures.push("provider-health capability-preflight source identity drifted");
for (const [object, key, expected] of [
  [contract.shared_runtime_policy, "owner", "rustok_page_builder::rollout::effective_provider_runtime_flags"],
  [contract.shared_runtime_policy, "configured_rollout_flags_remain_authoritative", true],
  [contract.shared_runtime_policy, "provider_health_may_only_narrow", true],
  [contract.graphql_preflight, "operation", "pageBuilderCapabilityPreflight"],
  [contract.graphql_preflight, "non_mutating", true],
  [contract.graphql_preflight, "provider_health_source", "PagesGraphqlRuntimeData"],
  [contract.graphql_preflight, "uses_shared_runtime_policy", true],
  [contract.graphql_preflight, "disabled_error_kind", "feature-disabled"],
  [contract.graphql_preflight, "disabled_error_code", "FEATURE_DISABLED"],
  [contract.graphql_preflight, "snapshot_continues_to_return_configured_flags_separately_from_health", true],
  [contract.rollout_feature_preflight_isolation, "existing_four_profile_harness_remains_rollout_only", true],
  [contract.rollout_feature_preflight_isolation, "existing_harness_fails_closed_if_health_is_observed", true],
  [contract.rollout_feature_preflight_isolation, "existing_harness_retains_raw_health_payload", false],
  [contract.anti_promotion, "observed_graphql_preflight_executed", false],
  [contract.anti_promotion, "pages_reference_consumer_gate_accepted", false],
]) if (object?.[key] !== expected) failures.push(`${key} must equal ${JSON.stringify(expected)}`);

for (const marker of [
  "pub fn effective_provider_runtime_flags(", "ProviderHealthState::Unavailable", "ProviderHealthState::Degraded", "effective.publish_enabled = false",
]) need(sources.coreRollout, marker, "shared runtime policy");
for (const marker of [
  "effective_provider_runtime_flags(&self.flags, self.health.as_ref())", "pub fn effective_runtime_flags(&self) -> BuilderCapabilityFlags",
]) need(sources.providerStatus, marker, "admin provider status delegation");
forbid(sources.providerStatus, "PageBuilderAdminProviderState::Unavailable => BuilderCapabilityFlags", "admin provider status must not duplicate runtime policy");

for (const marker of [
  "PagesGraphqlRuntimeData::provider_health_snapshot", "async fn page_builder_capability_preflight(",
  "let provider_health = provider_health_snapshot(ctx);", "effective_provider_runtime_flags(&flags, provider_health.as_ref())",
  "ensure_capability(&effective_flags, capability_kind)", "PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE",
]) need(sources.owner, marker, "Pages GraphQL provider-health preflight");
for (const marker of ["provider_health_authority", "authority.current_snapshot()"])
  need(sources.runtimeData, marker, "fresh runtime health authority");
need(sources.pagesSnapshot, "self.provider_status().effective_runtime_flags()", "Pages snapshot runtime policy");
for (const marker of ["let effective_flags = trusted_rollout.effective_runtime_flags();", "compose_fly_page_builder_handlers(store, renderer, effective_flags)"])
  need(sources.pagesFacade, marker, "authoritative SSR runtime policy");

if (
  rolloutContract.provider_health_boundary?.harness_purpose !== "rollout_only" ||
  rolloutContract.provider_health_boundary?.provider_health_must_remain_unobserved !== true ||
  rolloutContract.provider_health_boundary?.observe_before_each_profile_preflight !== true ||
  rolloutContract.provider_health_boundary?.observe_after_each_profile_preflight !== true ||
  rolloutContract.provider_health_boundary?.provider_health_payload_must_be_absent !== true
) failures.push("rollout-only feature-preflight health boundary drifted");
if (rolloutEvidence.source_contract?.provider_health_observed !== false || rolloutEvidence.source_contract?.raw_provider_health_payload_persisted !== false) failures.push("rollout-only harness must remain unobserved without raw health retention");
for (const marker of [
  "async function assertProviderHealthUnobserved(", "snapshot.providerHealthObserved !== false", "snapshot.providerHealth !== null",
  "provider_health_before: providerHealthBefore", "provider_health_after: providerHealthAfter",
]) need(sources.rolloutHarnessSpec, marker, "rollout-only live health isolation");

if (runtimeHarness.format !== "pages_builder_provider_health_runtime_harness_source_v1" || runtimeHarness.status !== "source_ready_maintainer_execution_pending") failures.push("observed-health runtime harness continuation is not source-ready");
if (consumerBinding.format !== "pages_builder_provider_health_consumer_binding_source_v1") failures.push("consumer binding predecessor drifted");
if (serverBinding.format !== "pages_builder_provider_health_server_binding_source_v1") failures.push("server binding predecessor drifted");
if (gate.accepted !== false || gate.current_boundary?.provider_health !== "unobserved") failures.push("retained Pages gate must remain unaccepted/provider-health-unobserved");

if (contract.next_cursor?.provider_health_capability_preflight !== "source_ready_runtime_execution_pending") failures.push("capability-preflight cursor drifted");
if (contract.next_cursor?.observed_health_runtime_evidence_harness !== "source_ready_maintainer_execution_pending") failures.push("observed-health runtime harness must be source-ready/maintainer-execution-pending");
if (contract.next_cursor?.observed_health_acceptance !== "pending") failures.push("observed-health acceptance must remain pending");

for (const marker of ["provider-health-capability-preflight-source-ready", "FEATURE_DISABLED", "runtime execution remains maintainer-owned", "Tests were not run"])
  need(sources.overlay, marker, "capability-preflight actualization");
for (const marker of ["provider-health-capability-preflight-source-ready", "observed-health runtime evidence harness [source-ready / maintainer execution pending]"])
  need(sources.parity, marker, "plan parity actualization");

if (failures.length) {
  console.error("[verify-pages-builder-provider-health-capability-preflight] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-builder-provider-health-capability-preflight] PASS source_ready=true runtime_harness=source_ready execution=pending");
