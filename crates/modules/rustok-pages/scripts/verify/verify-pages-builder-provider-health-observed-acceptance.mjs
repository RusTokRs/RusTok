#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  contract: "crates/modules/rustok-pages/contracts/evidence/pages-builder-provider-health-observed-acceptance-source.json",
  runner: "scripts/evidence/accept-pages-builder-provider-health-runtime.mjs",
  runtimeContract: "crates/modules/rustok-pages/contracts/evidence/pages-builder-provider-health-runtime-execution-contract.json",
  runtimeSource: "crates/modules/rustok-pages/contracts/evidence/pages-builder-provider-health-runtime-harness-source.json",
  identitySource: "crates/modules/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-identity-source.json",
  evaluatorSource: "crates/modules/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-evaluator-source.json",
  bindingSource: "crates/modules/rustok-pages/contracts/evidence/pages-builder-provider-health-owner-acceptance-source.json",
  runtimeVerifier: "crates/modules/rustok-pages/scripts/verify/verify-pages-builder-provider-health-runtime-harness.mjs",
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
  console.error("[verify-pages-builder-provider-health-observed-acceptance] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const sources = Object.fromEntries(Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]));
const contract = JSON.parse(sources.contract);
const runtimeContract = JSON.parse(sources.runtimeContract);
const runtimeSource = JSON.parse(sources.runtimeSource);
const identitySource = JSON.parse(sources.identitySource);
const evaluatorSource = JSON.parse(sources.evaluatorSource);
const bindingSource = JSON.parse(sources.bindingSource);
const gate = JSON.parse(sources.gate);

if (contract.format !== "pages_builder_provider_health_observed_acceptance_source_v1" || contract.status !== "source_ready_maintainer_execution_pending") failures.push("observed-health acceptance source identity drifted");
if (runtimeContract.packet !== "pages-builder-provider-health-runtime-evidence" || runtimeContract.output?.format !== "pages_builder_provider_health_runtime_evidence_v1" || runtimeContract.output?.status !== "observed_runtime_evidence_owner_review_pending") failures.push("runtime evidence predecessor contract drifted");
if (runtimeSource.format !== "pages_builder_provider_health_runtime_harness_source_v1" || runtimeSource.status !== "source_ready_maintainer_execution_pending") failures.push("runtime harness predecessor source drifted");
if (identitySource.format !== "page_builder_provider_health_deployment_identity_source_v1") failures.push("identity source contract drifted");
if (evaluatorSource.format !== "page_builder_provider_health_deployment_evaluator_source_v1") failures.push("evaluator source contract drifted");
if (bindingSource.format !== "pages_builder_provider_health_owner_acceptance_source_v1") failures.push("binding owner-acceptance source contract drifted");

for (const [object, key, expected] of [
  [contract.runtime_evidence_input, "format", "pages_builder_provider_health_runtime_evidence_v1"],
  [contract.runtime_evidence_input, "required_status", "observed_runtime_evidence_owner_review_pending"],
  [contract.runtime_evidence_input, "source_commit_must_equal_checkout_head", true],
  [contract.runtime_evidence_input, "source_hashes_must_match_runtime_execution_contract_and_checkout", true],
  [contract.runtime_evidence_input, "identity_evaluation_binding_acceptance_hashes_must_match_supplied_packets", true],
  [contract.runtime_evidence_input, "supplied_predecessor_source_hashes_must_match_contracts_and_checkout", true],
  [contract.runtime_evidence_input, "health_may_be_expired_at_owner_review", true],
  [contract.runtime_evidence_input, "health_lease_must_not_be_restarted_or_extended", true],
  [contract.runtime_evidence_input, "configured_rollout_all_on_required", true],
  [contract.runtime_evidence_input, "graphql_provider_health_observed_required", true],
  [contract.runtime_evidence_input, "graphql_http_status_and_body_hashes_revalidated", true],
  [contract.runtime_evidence_input, "canonical_feature_disabled_contract_required_when_narrowed", true],
  [contract.runtime_evidence_input, "publish_mutation_executed_must_be_false", true],
  [contract.runtime_evidence_input, "rollout_settings_mutated_must_be_false", true],
  [contract.owner_decision, "acceptance_does_not_assert_current_provider_health", true],
  [contract.owner_decision, "acceptance_does_not_authorize_or_extend_server_binding", true],
  [contract.owner_decision, "acceptance_does_not_accept_pages_reference_consumer_gate", true],
  [contract.output, "accepted_status", "owner_accepted_observed_runtime_evidence_gate_review_pending"],
  [contract.output, "rejected_status", "owner_rejected_observed_runtime_evidence"],
  [contract.gate_boundary, "accepted_packet_is_eligible_input_for_pages_gate_review", true],
  [contract.gate_boundary, "accepted_packet_does_not_accept_pages_gate", true],
  [contract.gate_boundary, "accepted_packet_does_not_satisfy_reference_gate_owner_signoff_or_rollback_by_itself", true],
  [contract.non_claims, "observed_health_owner_acceptance_executed", false],
  [contract.non_claims, "current_provider_health_asserted", false],
  [contract.non_claims, "health_lease_extended", false],
  [contract.non_claims, "server_binding_changed", false],
  [contract.non_claims, "pages_reference_consumer_gate_accepted", false],
  [contract.non_claims, "tests_run", false],
  [contract.non_claims, "node_verifier_run", false],
]) if (object?.[key] !== expected) failures.push(`${key} must equal ${JSON.stringify(expected)}`);

for (const [node, expectedPath, label] of [
  [contract.supplied_predecessor_packets?.deployment_identity, files.identitySource, "identity"],
  [contract.supplied_predecessor_packets?.deployment_evaluation, files.evaluatorSource, "evaluation"],
  [contract.supplied_predecessor_packets?.binding_owner_acceptance, files.bindingSource, "binding acceptance"],
]) if (node?.source_contract !== expectedPath) failures.push(`${label} source-contract path drifted`);
if (contract.supplied_predecessor_packets?.binding_owner_acceptance?.decision !== "accept_for_pages_binding" || contract.supplied_predecessor_packets?.binding_owner_acceptance?.rollback_action !== "restore_unobserved_provider_health") failures.push("binding acceptance lineage drifted");
if (!contract.owner_decision?.decisions?.includes("accept_observed_runtime_evidence") || !contract.owner_decision?.decisions?.includes("reject")) failures.push("observed-health owner decision set drifted");

, marker, "observed-health acceptance actualization");
, marker, "plan parity actualization");

if (contract.next_cursor?.observed_health_owner_acceptance !== "source_ready_maintainer_execution_pending") failures.push("observed-health acceptance source cursor drifted");
if (contract.next_cursor?.pages_reference_consumer_gate_acceptance !== "source_ready_maintainer_execution_pending") failures.push("Pages reference-consumer gate acceptance must be source-ready and maintainer-execution-pending");

if (failures.length) {
  console.error("[verify-pages-builder-provider-health-observed-acceptance] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-builder-provider-health-observed-acceptance] PASS source_ready=true execution=pending gate_acceptance=source_ready");
