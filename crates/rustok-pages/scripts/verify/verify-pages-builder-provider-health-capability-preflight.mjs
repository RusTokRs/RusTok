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
  rolloutHarnessEvidence: "crates/rustok-pages/contracts/evidence/pages-builder-rollout-feature-preflight-harness-source.json",
  consumerBinding: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-consumer-binding-source.json",
  serverBinding: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-server-binding-source.json",
  gate: "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json",
  overlay: "docs/modules/pages-page-builder-provider-health-capability-preflight-actualization-2026-08-09.md",
  parity: "docs/modules/pages-page-builder-plan-parity-actualization-2026-08-08.md",
};

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
if (failures.length) {
  console.error("[verify-pages-builder-provider-health-capability-preflight] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const contract = JSON.parse(sources.contract);
const rolloutHarnessEvidence = JSON.parse(sources.rolloutHarnessEvidence);
const consumerBinding = JSON.parse(sources.consumerBinding);
const serverBinding = JSON.parse(sources.serverBinding);
const gate = JSON.parse(sources.gate);

if (contract.format !== "pages_builder_provider_health_capability_preflight_source_v1") {
  failures.push("provider-health capability preflight contract format drifted");
}
if (contract.status !== "source_ready_runtime_execution_pending") {
  failures.push("provider-health capability preflight must remain source-ready/runtime-execution-pending");
}
for (const [object, key, expected] of [
  [contract.shared_runtime_policy, "owner", "rustok_page_builder::rollout::effective_provider_runtime_flags"],
  [contract.shared_runtime_policy, "configured_rollout_flags_remain_authoritative", true],
  [contract.shared_runtime_policy, "provider_health_may_only_narrow", true],
  [contract.shared_runtime_policy, "admin_provider_status_delegates_to_shared_policy", true],
  [contract.graphql_preflight, "operation", "pageBuilderCapabilityPreflight"],
  [contract.graphql_preflight, "non_mutating", true],
  [contract.graphql_preflight, "provider_health_source", "PagesGraphqlRuntimeData"],
  [contract.graphql_preflight, "fresh_authority_read_per_preflight_request", true],
  [contract.graphql_preflight, "uses_shared_runtime_policy", true],
  [contract.graphql_preflight, "disabled_error_kind", "feature-disabled"],
  [contract.graphql_preflight, "disabled_error_code", "FEATURE_DISABLED"],
  [contract.graphql_preflight, "degraded_all_on_publish_is_feature_disabled", true],
  [contract.graphql_preflight, "unavailable_all_on_preview_is_feature_disabled", true],
  [contract.graphql_preflight, "snapshot_continues_to_return_configured_flags_separately_from_health", true],
  [contract.rollout_feature_preflight_isolation, "existing_four_profile_harness_remains_rollout_only", true],
  [contract.rollout_feature_preflight_isolation, "existing_harness_claims_provider_health_observed", false],
  [contract.anti_promotion, "observed_graphql_preflight_executed", false],
  [contract.anti_promotion, "pages_reference_consumer_gate_accepted", false],
  [contract.validation, "tests_run", false],
  [contract.validation, "node_verifier_run", false],
]) {
  if (object?.[key] !== expected) failures.push(`${key} must equal ${JSON.stringify(expected)}`);
}

for (const marker of [
  "pub fn effective_provider_runtime_flags(",
  "flags.validate().is_err()",
  "ProviderHealthState::Unavailable",
  "ProviderHealthState::Degraded",
  "effective.publish_enabled = false",
  "provider_health_runtime_flags_only_narrow_configured_rollout",
]) need(sources.coreRollout, marker, "shared Page Builder runtime policy");

for (const marker of [
  "effective_provider_runtime_flags",
  "pub fn effective_runtime_flags(&self) -> BuilderCapabilityFlags",
  "effective_provider_runtime_flags(&self.flags, self.health.as_ref())",
]) need(sources.providerStatus, marker, "admin provider status delegation");
forbid(
  sources.providerStatus,
  "PageBuilderAdminProviderState::Unavailable => BuilderCapabilityFlags",
  "admin provider status must not retain a second runtime-flag policy",
);

for (const marker of [
  "fn provider_health_snapshot(ctx: &Context<'_>) -> Option<ProviderHealthSnapshot>",
  "PagesGraphqlRuntimeData::provider_health_snapshot",
  "async fn page_builder_capability_preflight(",
  "let required_permission = required_page_builder_permission(capability_kind);",
  "let flags = load_rollout_flags(db, tenant).await?;",
  "let provider_health = provider_health_snapshot(ctx);",
  "effective_provider_runtime_flags(&flags, provider_health.as_ref())",
  "ensure_capability(&effective_flags, capability_kind)",
  "PageBuilderErrorKind::FeatureDisabled.as_str()",
  "PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE",
]) need(sources.owner, marker, "Pages GraphQL provider-health preflight");

const snapshotStart = sources.owner.indexOf("async fn page_builder_rollout_snapshot(");
const preflightStart = sources.owner.indexOf("async fn page_builder_capability_preflight(");
const mappingStart = sources.owner.indexOf("\nfn required_page_builder_permission(", preflightStart);
if (snapshotStart < 0 || preflightStart <= snapshotStart || mappingStart <= preflightStart) {
  failures.push("GraphQL rollout/preflight source slices could not be isolated");
} else {
  const snapshotSlice = sources.owner.slice(snapshotStart, preflightStart);
  const preflightSlice = sources.owner.slice(preflightStart, mappingStart);
  forbid(snapshotSlice, "effective_provider_runtime_flags", "rollout snapshot must expose configured flags separately from health");
  for (const marker of ["save_project(", "render_preview(", ".publish(", "save_document(", "std::fs::", "RUSTOK_PAGES_PROVIDER_HEALTH_ACCEPTANCE_PATH"]) {
    forbid(preflightSlice, marker, "non-mutating provider-health capability preflight");
  }
}

for (const marker of [
  "provider_health_authority",
  "authority.current_snapshot()",
]) need(sources.runtimeData, marker, "fresh GraphQL runtime health authority");

for (const marker of [
  "self.provider_status().effective_runtime_flags()",
]) need(sources.pagesSnapshot, marker, "Pages snapshot runtime-policy delegation");
for (const marker of [
  "let effective_flags = trusted_rollout.effective_runtime_flags();",
  "compose_fly_page_builder_handlers(store, renderer, effective_flags)",
]) need(sources.pagesFacade, marker, "authoritative SSR shared-policy path");

if (rolloutHarnessEvidence.source_contract?.provider_health_observed !== false) {
  failures.push("existing four-profile feature preflight harness must remain provider-health unobserved");
}
if (consumerBinding.format !== "pages_builder_provider_health_consumer_binding_source_v1") {
  failures.push("consumer-binding predecessor format drifted");
}
if (serverBinding.format !== "pages_builder_provider_health_server_binding_source_v1") {
  failures.push("server-binding predecessor format drifted");
}
if (gate.accepted !== false || gate.current_boundary?.provider_health !== "unobserved") {
  failures.push("Pages reference-consumer gate must remain unaccepted/provider-health-unobserved in retained execution evidence");
}

for (const marker of [
  "provider-health-capability-preflight-source-ready",
  "shared Page Builder runtime policy",
  "pageBuilderCapabilityPreflight",
  "FEATURE_DISABLED",
  "runtime execution remains maintainer-owned",
  "Tests were not run",
]) need(sources.overlay, marker, "provider-health preflight actualization");
for (const marker of [
  "provider-health-capability-preflight-source-ready",
  "pages-page-builder-provider-health-capability-preflight-actualization-2026-08-09.md",
  "health-aware non-mutating capability preflight [source-ready]",
]) need(sources.parity, marker, "plan parity actualization");

if (contract.next_cursor?.provider_health_capability_preflight !== "source_ready_runtime_execution_pending") {
  failures.push("provider-health capability preflight cursor drifted");
}
if (contract.next_cursor?.observed_health_runtime_evidence_harness !== "source_open") {
  failures.push("observed-health runtime evidence harness must remain the next source cursor");
}
if (contract.next_cursor?.observed_health_acceptance !== "pending") {
  failures.push("observed-health acceptance must remain pending");
}

if (failures.length) {
  console.error("[verify-pages-builder-provider-health-capability-preflight] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-builder-provider-health-capability-preflight] PASS source_ready=true runtime_execution=pending");
