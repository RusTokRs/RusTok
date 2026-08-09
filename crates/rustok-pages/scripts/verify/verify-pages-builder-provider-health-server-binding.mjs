#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  contract: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-server-binding-source.json",
  binding: "crates/rustok-pages/src/provider_health_binding.rs",
  lib: "crates/rustok-pages/src/lib.rs",
  manifest: "crates/rustok-pages/rustok-module.toml",
  graphqlMod: "crates/rustok-pages/src/graphql/mod.rs",
  runtimeData: "crates/rustok-pages/src/graphql/runtime_data.rs",
  owner: "crates/rustok-pages/src/graphql/builder_rollout.rs",
  composition: "crates/rustok-pages/admin/src/composition.rs",
  builder: "crates/rustok-pages/admin/src/builder.rs",
  adminMain: "apps/admin/src/main.rs",
  acceptance: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-owner-acceptance-source.json",
  acceptanceRunner: "scripts/evidence/accept-pages-builder-provider-health-deployment.mjs",
  transport: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-transport-source.json",
  overlay: "docs/modules/pages-page-builder-provider-health-server-binding-actualization-2026-08-09.md",
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

const sources = failures.length === 0
  ? Object.fromEntries(Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]))
  : {};
let contract = {};
let acceptance = {};
let transport = {};
if (failures.length === 0) {
  for (const [label, assign] of [
    ["contract", (value) => (contract = value)],
    ["acceptance", (value) => (acceptance = value)],
    ["transport", (value) => (transport = value)],
  ]) {
    try {
      assign(JSON.parse(sources[label]));
    } catch (error) {
      failures.push(`${label}: invalid JSON: ${error.message}`);
    }
  }
}

if (contract.format !== "pages_builder_provider_health_server_binding_source_v1") {
  failures.push("server binding contract format drifted");
}
if (contract.status !== "source_ready_maintainer_activation_pending") {
  failures.push("server binding must remain source-ready with maintainer activation pending");
}
if (acceptance.format !== "pages_builder_provider_health_owner_acceptance_source_v1") {
  failures.push("owner acceptance predecessor format drifted");
}
if (transport.format !== "pages_builder_provider_health_transport_source_v1") {
  failures.push("typed transport predecessor format drifted");
}

for (const [object, key, expected] of [
  [contract.configuration, "acceptance_path_env", "RUSTOK_PAGES_PROVIDER_HEALTH_ACCEPTANCE_PATH"],
  [contract.configuration, "deployment_id_env", "RUSTOK_PAGES_PROVIDER_HEALTH_DEPLOYMENT_ID"],
  [contract.configuration, "deployment_image_digest_env", "RUSTOK_PAGES_PROVIDER_HEALTH_DEPLOYMENT_IMAGE_DIGEST"],
  [contract.configuration, "source_commit_env", "RUSTOK_SOURCE_COMMIT"],
  [contract.configuration, "acceptance_path_must_be_absolute", true],
  [contract.configuration, "all_pages_binding_values_absent_means_unobserved", true],
  [contract.configuration, "invalid_configuration_fails_application_startup", false],
  [contract.accepted_packet, "regular_non_symlink_file_required_per_read", true],
  [contract.accepted_packet, "strict_unknown_fields_rejected", true],
  [contract.accepted_packet, "decision", "accept_for_pages_binding"],
  [contract.accepted_packet, "rollback_action", "restore_unobserved_provider_health"],
  [contract.accepted_packet, "maximum_target_operation_freshness_age_seconds_required", true],
  [contract.accepted_packet, "health_valid_until_required", true],
  [contract.live_identity_binding, "source_commit_must_equal_live_environment", true],
  [contract.live_identity_binding, "deployment_id_must_equal_live_environment", true],
  [contract.live_identity_binding, "deployment_image_digest_must_equal_live_environment", true],
  [contract.evidence_revalidation, "minimum_preview_samples", 20],
  [contract.evidence_revalidation, "minimum_publish_samples", 20],
  [contract.evidence_revalidation, "maximum_target_operation_freshness_age_must_not_exceed_freshness", true],
  [contract.evidence_revalidation, "health_valid_until_recomputed_from_remaining_freshness", true],
  [contract.evidence_revalidation, "accepted_decision_must_not_exceed_health_valid_until_plus_skew", true],
  [contract.evidence_revalidation, "canonical_provider_health_snapshot_recomputed", true],
  [contract.evidence_revalidation, "canonical_slo_evaluation_recomputed", true],
  [contract.freshness_lease, "checked_on_every_rollout_snapshot_read", true],
  [contract.freshness_lease, "future_clock_skew_tolerance_seconds", 5],
  [contract.freshness_lease, "maximum_observed_until", "evaluation.health_valid_until + clock_skew_tolerance"],
  [contract.freshness_lease, "expired_action", "unobserved"],
  [contract.hot_replacement, "packet_reloaded_on_every_rollout_snapshot_read", true],
  [contract.hot_replacement, "accepted_to_rejected_transition_requires_restart", false],
  [contract.hot_replacement, "rejected_to_accepted_transition_requires_restart", false],
  [contract.composition, "module_registration", "PagesModule::register_runtime_extensions"],
  [contract.composition, "graphql_manifest_runtime_data_factory", "graphql::attach_schema_data"],
  [contract.current_consumer_boundary, "workspace_uses_health_for_capabilities", false],
  [contract.current_consumer_boundary, "authoritative_ssr_uses_health_for_capabilities", false],
  [contract.current_consumer_boundary, "standalone_browser_intent_uses_health_for_capabilities", false],
  [contract.non_claims, "server_binding_activated_with_live_packet", false],
  [contract.non_claims, "graphql_observed_health_request_executed", false],
  [contract.non_claims, "pages_reference_consumer_gate_accepted", false],
  [contract.non_claims, "forum_wave_accepted", false],
  [contract.non_claims, "tests_run", false],
  [acceptance.evaluation_input, "maximum_target_operation_freshness_age_recomputed", true],
  [acceptance.evaluation_input, "health_valid_until_derived_from_remaining_freshness", true],
  [acceptance.owner_decision, "accept_requires_unexpired_health_valid_until", true],
  [acceptance.owner_decision, "acceptance_clock_skew_tolerance_seconds", 5],
  [acceptance.output, "maximum_target_operation_freshness_age_seconds_retained", true],
  [acceptance.output, "health_valid_until_retained", true],
]) {
  if (object?.[key] !== expected) failures.push(`${key} must equal ${JSON.stringify(expected)}`);
}

for (const marker of [
  '"RUSTOK_PAGES_PROVIDER_HEALTH_ACCEPTANCE_PATH"',
  '"RUSTOK_PAGES_PROVIDER_HEALTH_DEPLOYMENT_ID"',
  '"RUSTOK_PAGES_PROVIDER_HEALTH_DEPLOYMENT_IMAGE_DIGEST"',
  '"RUSTOK_SOURCE_COMMIT"',
  'const ACCEPTED_STATUS: &str = "owner_accepted_server_binding_pending"',
  'const ACCEPT_DECISION: &str = "accept_for_pages_binding"',
  'const ROLLBACK_ACTION: &str = "restore_unobserved_provider_health"',
  "PagesProviderHealthLiveIdentity",
  "PagesProviderHealthAuthoritySource::RetainedPacket",
  "from_retained_packet_path",
  "read_retained_packet(path)",
  "fs::symlink_metadata",
  "ErrorKind::NotFound",
  "MAX_ACCEPTANCE_PACKET_BYTES",
  "#[serde(deny_unknown_fields)]",
  "max_target_operation_freshness_age_seconds",
  "health_valid_until",
  "expected_valid_until",
  "ProviderHealthSnapshot::evaluate(packet.evaluation.snapshot.observed)",
  "ProviderSloEvaluation::evaluate(",
  "snapshot_at(Utc::now())",
  "now > self.health_valid_until + skew",
  "IncompleteEnvironment",
  "IdentityMismatch",
  "HealthPolicyMismatch",
]) need(sources.binding ?? "", marker, "provider health authority");

for (const marker of [
  "maximumFreshnessAge",
  "remainingFreshnessSeconds",
  "healthValidUntilMs",
  "health_valid_until: admitted.healthValidUntil",
  "max_target_operation_freshness_age_seconds: admitted.maximumFreshnessAge",
  "accepted decision is outside the retained health freshness deadline",
]) need(sources.acceptanceRunner ?? "", marker, "owner acceptance remaining freshness");

for (const marker of [
  "page_builder_provider_health_authority_from_environment()",
  "extensions.insert(authority)",
  "provider health remains unobserved",
  "PAGES_PROVIDER_HEALTH_ACCEPTANCE_PATH_ENV",
]) need(sources.lib ?? "", marker, "Pages module host composition");

need(
  sources.manifest ?? "",
  'runtime_data_factory = "graphql::attach_schema_data"',
  "Pages manifest GraphQL runtime data",
);
need(sources.graphqlMod ?? "", "pub use runtime_data::attach_schema_data", "Pages GraphQL runtime factory export");
need(sources.graphqlMod ?? "", "pub use runtime_data::PagesGraphqlRuntimeData", "Pages GraphQL runtime data export");
for (const marker of [
  "PagesGraphqlRuntimeData",
  "inputs.shared_get::<SharedPagesProviderHealthAuthority>()",
  "authority.current_snapshot()",
]) need(sources.runtimeData ?? "", marker, "Pages GraphQL runtime data");

for (const marker of [
  "provider_health_observed: false",
  "provider_health: None",
  "data_opt::<PagesGraphqlRuntimeData>()",
  "PagesGraphqlRuntimeData::provider_health_snapshot",
  "Some(health) => snapshot.with_provider_health(&health)",
  "None => snapshot",
]) need(sources.owner ?? "", marker, "Pages rollout query server binding");
forbid(sources.owner ?? "", "std::fs::", "GraphQL query must consume runtime authority rather than files directly");
forbid(sources.owner ?? "", "RUSTOK_PAGES_PROVIDER_HEALTH_ACCEPTANCE_PATH", "GraphQL query must not own host configuration");

// Server binding is source-ready, but capability consumers remain rollout-only in this slice.
for (const marker of [
  ".flags;",
  "provider_flags: BuilderCapabilityFlags",
  ".with_provider_flags(provider_flags)",
]) need(sources.composition ?? "", marker, "workspace remains rollout-only");
need(sources.builder ?? "", ".map(PageBuilderAdminProviderStatus::unobserved)", "SSR facade remains rollout-only");
for (const marker of ["pages_editor_capabilities_for_rollout(", "&rollout.flags"])
  need(sources.adminMain ?? "", marker, "browser intent remains rollout-only");
for (const source of [sources.composition ?? "", sources.builder ?? "", sources.adminMain ?? ""]) {
  forbid(source, "pages_editor_capabilities_for_snapshot(", "health-driven capability binding remains next slice");
}

for (const marker of [
  "server-provider-health-binding-source-ready",
  "hot-accept-reject-source-ready",
  "freshness-lease-source-ready",
  "health_valid_until",
  "restore_unobserved_provider_health",
  "runtime_data_factory",
  "Pages remains `unobserved`",
  "Tests were not run",
]) need(sources.overlay ?? "", marker, "server binding actualization");

for (const marker of [
  "provider-health-server-binding-source-ready",
  "pages-page-builder-provider-health-server-binding-actualization-2026-08-09.md",
  "server provider-health binding",
  "UI / SSR / browser-intent provider-health binding",
]) need(sources.parity ?? "", marker, "plan parity actualization");

if (contract.next_cursor?.server_provider_health_binding !== "source_ready_maintainer_activation_pending") {
  failures.push("server binding cursor drifted");
}
if (contract.next_cursor?.retained_identity_evaluator_owner_acceptance !== "maintainer_execution_pending") {
  failures.push("retained runtime evidence cursor drifted");
}
if (contract.next_cursor?.ui_ssr_browser_intent_health_binding !== "source_open_runtime_activation_blocked") {
  failures.push("consumer binding cursor drifted");
}

if (failures.length > 0) {
  console.error("[verify-pages-builder-provider-health-server-binding] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log("[verify-pages-builder-provider-health-server-binding] PASS server_binding=source_ready activation=pending consumer_binding=open");
