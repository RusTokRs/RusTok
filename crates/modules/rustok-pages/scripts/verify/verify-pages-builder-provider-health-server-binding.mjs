#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  contract: "crates/modules/rustok-pages/contracts/evidence/pages-builder-provider-health-server-binding-source.json",
  consumer: "crates/modules/rustok-pages/contracts/evidence/pages-builder-provider-health-consumer-binding-source.json",
  binding: "crates/modules/rustok-pages/src/provider_health_binding.rs",
  lib: "crates/modules/rustok-pages/src/lib.rs",
  manifest: "crates/modules/rustok-pages/rustok-module.toml",
  graphqlMod: "crates/modules/rustok-pages/src/graphql/mod.rs",
  runtimeData: "crates/modules/rustok-pages/src/graphql/runtime_data.rs",
  owner: "crates/modules/rustok-pages/src/graphql/builder_rollout.rs",
  composition: "crates/modules/rustok-pages/admin/src/composition.rs",
  builder: "crates/modules/rustok-pages/admin/src/builder.rs",
  adminMain: "apps/admin/src/main.rs",
  acceptance: "crates/modules/rustok-pages/contracts/evidence/pages-builder-provider-health-owner-acceptance-source.json",
  acceptanceRunner: "scripts/evidence/accept-pages-builder-provider-health-deployment.mjs",
  transport: "crates/modules/rustok-pages/contracts/evidence/pages-builder-provider-health-transport-source.json",
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
let consumer = {};
let acceptance = {};
let transport = {};
if (failures.length === 0) {
  for (const [label, assign] of [
    ["contract", (value) => (contract = value)],
    ["consumer", (value) => (consumer = value)],
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
if (consumer.format !== "pages_builder_provider_health_consumer_binding_source_v1") {
  failures.push("consumer binding continuation format drifted");
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
  [contract.current_consumer_boundary, "workspace_uses_health_for_capabilities", true],
  [contract.current_consumer_boundary, "workspace_exposes_observed_provider_status", true],
  [contract.current_consumer_boundary, "authoritative_ssr_uses_health_for_capabilities", true],
  [contract.current_consumer_boundary, "standalone_browser_intent_uses_health_for_capabilities", true],
  [contract.current_consumer_boundary, "canonical_consumer_binding_contract", "pages_builder_provider_health_consumer_binding_source_v1"],
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

?? "", marker, "server binding actualization");

?? "", marker, "plan parity actualization");

if (contract.next_cursor?.server_provider_health_binding !== "source_ready_maintainer_activation_pending") {
  failures.push("server binding cursor drifted");
}
if (contract.next_cursor?.ui_ssr_browser_intent_health_binding !== "source_ready_runtime_activation_pending") {
  failures.push("consumer binding cursor drifted");
}
if (contract.next_cursor?.retained_identity_evaluator_owner_acceptance !== "maintainer_execution_pending") {
  failures.push("retained runtime evidence cursor drifted");
}

if (failures.length > 0) {
  console.error("[verify-pages-builder-provider-health-server-binding] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log("[verify-pages-builder-provider-health-server-binding] PASS server_binding=source_ready consumer_binding=source_ready activation=pending");
