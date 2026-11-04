#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  contract: "crates/modules/rustok-pages/contracts/evidence/pages-builder-provider-health-consumer-binding-source.json",
  coreRollout: "crates/modules/rustok-page-builder/src/rollout.rs",
  providerStatus: "crates/modules/rustok-page-builder/admin/src/provider_status.rs",
  owner: "crates/modules/rustok-pages/src/graphql/builder_rollout.rs",
  rollout: "crates/modules/rustok-pages/admin/src/builder_rollout_settings.rs",
  facade: "crates/modules/rustok-pages/admin/src/builder.rs",
  composition: "crates/modules/rustok-pages/admin/src/composition.rs",
  adminMain: "apps/admin/src/main.rs",
  serverBinding: "crates/modules/rustok-pages/contracts/evidence/pages-builder-provider-health-server-binding-source.json",
  transport: "crates/modules/rustok-pages/contracts/evidence/pages-builder-provider-health-transport-source.json",
  preflightEvidence: "crates/modules/rustok-pages/contracts/evidence/pages-builder-provider-health-capability-preflight-source.json",
  runtimeHarness: "crates/modules/rustok-pages/contracts/evidence/pages-builder-provider-health-runtime-harness-source.json",
  gate: "crates/modules/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json",
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

, marker, "consumer binding actualization");
, marker, "canonical parity");

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
