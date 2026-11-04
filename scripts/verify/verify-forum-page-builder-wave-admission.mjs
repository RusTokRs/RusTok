#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const failures = [];
const files = {
  contract: "crates/modules/rustok-forum/contracts/evidence/forum-page-builder-wave-admission-source.json",
  runner: "scripts/evidence/admit-forum-page-builder-wave.mjs",
  wave: "crates/modules/rustok-forum/contracts/evidence/forum-wave1-rollout-evidence.json",
  gate: "crates/modules/rustok-pages/contracts/evidence/pages-reference-consumer-gate-acceptance-source.json",
  browser: "crates/modules/rustok-forum/contracts/evidence/forum-page-builder-browser-execution-contract.json",
  runtime: "crates/modules/rustok-forum/contracts/evidence/forum-page-builder-runtime-authorization-execution-contract.json",
  serverfn: "crates/modules/rustok-forum/contracts/evidence/forum-page-builder-serverfn-deployment-attestation-contract.json",
  freshness: "scripts/verify/verify-forum-wave-evidence-freshness.mjs",
  freshnessTest: "scripts/verify/verify-forum-wave-evidence-freshness.test.mjs",
  planSync: "scripts/verify/verify-forum-wave-plan-sync.mjs",
  planSyncTest: "scripts/verify/verify-forum-wave-plan-sync.test.mjs",
};
const admissionVerifierPath = "scripts/verify/verify-forum-page-builder-wave-admission.mjs";
const abs = (value) => path.join(repoRoot, value);
const read = (value) => fs.readFileSync(abs(value), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};

for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(abs(relativePath))) {
    failures.push(`${label}: missing ${relativePath}`);
    continue;
  }
  const stats = fs.lstatSync(abs(relativePath));
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${label}: ${relativePath} must be a regular non-symlink file`);
  }
}
if (failures.length) {
  console.error("[verify-forum-page-builder-wave-admission] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const contract = JSON.parse(read(files.contract));
const wave = JSON.parse(read(files.wave));
const gate = JSON.parse(read(files.gate));
const browser = JSON.parse(read(files.browser));
const runtime = JSON.parse(read(files.runtime));
const serverfn = JSON.parse(read(files.serverfn));
const runner = read(files.runner);
const freshness = read(files.freshness);
const planSync = read(files.planSync);

if (
  contract.format !== "forum_page_builder_wave_admission_source_v1" ||
  contract.status !== "source_ready_maintainer_execution_pending" ||
  contract.module !== "forum" ||
  contract.wave !== "1"
) failures.push("Wave admission source identity drifted");

for (const [object, key, expected] of [
  [contract.pages_gate_input, "format", "pages_reference_consumer_gate_acceptance_v1"],
  [contract.pages_gate_input, "required_status", "owner_accepted_pages_reference_consumer_gate"],
  [contract.pages_gate_input, "required_decision", "accept_pages_reference_consumer_gate"],
  [contract.pages_gate_input, "required_rollback_decision", "retain_reference_consumer_candidate"],
  [contract.pages_gate_input, "retained_source_hashes_must_match_contract_and_checkout", true],
  [contract.forum_browser_input, "format", "forum_page_builder_browser_execution_v1"],
  [contract.forum_browser_input, "deployment_digest_must_equal_pages_gate", true],
  [contract.forum_browser_input, "all_required_profiles_must_pass", true],
  [contract.forum_runtime_authorization_input, "format", "forum_page_builder_runtime_authorization_execution_v1"],
  [contract.forum_runtime_authorization_input, "command_records_must_match_execution_contract_exactly", true],
  [contract.forum_serverfn_attestation_input, "format", "forum_page_builder_server_fn_deployment_attestation_v1"],
  [contract.forum_serverfn_attestation_input, "deployment_digest_must_equal_pages_gate_and_browser", true],
  [contract.lineage, "same_exact_source_commit_required_across_all_packets", true],
  [contract.lineage, "same_immutable_repo_digest_required_across_pages_gate_browser_and_serverfn", true],
  [contract.lineage, "runtime_authorization_is_source_bound_but_not_separately_deployment_bound", true],
  [contract.lineage, "browser_and_serverfn_deployment_identity_remains_maintainer_asserted_or_reviewed_where_contract_declares_it", true],
  [contract.lineage, "deployment_digest_equality_does_not_upgrade_origin_binding_to_cryptographic_proof", true],
  [contract.lineage, "accepted_pages_gate_is_a_precondition_not_a_forum_wave_acceptance", true],
  [contract.output, "format", "forum_page_builder_wave_admission_v1"],
  [contract.output, "status", "forum_wave_inputs_admitted_observed_control_plane_pending"],
  [contract.output, "cryptographic_deployment_binding_claimed", false],
  [contract.observed_wave_boundary, "admission_packet_must_be_bound_by_live_wave_evidence", true],
  [contract.observed_wave_boundary, "observed_control_plane_wave_not_executed", true],
  [contract.observed_wave_boundary, "current_provider_health_not_asserted_by_admission", true],
  [contract.observed_wave_boundary, "forum_wave_not_accepted", true],
  [contract.non_claims, "wave_admission_runner_executed", false],
  [contract.non_claims, "observed_control_plane_wave_executed", false],
  [contract.non_claims, "cryptographic_origin_to_repo_digest_binding_asserted", false],
  [contract.non_claims, "tests_run", false],
]) {
  if (object?.[key] !== expected) failures.push(`${key} drifted`);
}

if (gate.output?.format !== "pages_reference_consumer_gate_acceptance_v1" || gate.output?.accepted_status !== "owner_accepted_pages_reference_consumer_gate") {
  failures.push("Pages gate acceptance predecessor drifted");
}
if (gate.next_cursor?.forum_wave_admission !== "source_ready_maintainer_execution_pending") {
  failures.push("Pages gate predecessor does not point to Forum Wave admission");
}
if (browser.output?.format !== "forum_page_builder_browser_execution_v1" || browser.output?.status !== "browser_execution_passed_runtime_evidence_pending") {
  failures.push("Forum browser predecessor drifted");
}
if (runtime.output?.format !== "forum_page_builder_runtime_authorization_execution_v1" || runtime.output?.status !== "runtime_authorization_execution_passed_wave_pending") {
  failures.push("Forum runtime predecessor drifted");
}
if (serverfn.output?.format !== "forum_page_builder_server_fn_deployment_attestation_v1" || serverfn.output?.status !== "server_fn_deployment_attestation_passed_wave_pending") {
  failures.push("Forum server-function predecessor drifted");
}

if (
  wave.mode !== "source_ready" ||
  wave.provenance !== "synthetic_fixture" ||
  wave.execution_status !== "not_run_by_implementation_agent" ||
  wave.observed_run?.status !== "not_run" ||
  wave.observed_run?.blocked_by !== "pages_reference_consumer_gate"
) failures.push("canonical Forum Wave source must remain synthetic/unexecuted and Pages-gated");
if (
  wave.observed_run?.accepted_gate_evidence?.format !== "pages_reference_consumer_gate_acceptance_v1" ||
  wave.observed_run?.accepted_gate_evidence?.status !== "owner_accepted_pages_reference_consumer_gate" ||
  wave.observed_run?.accepted_gate_evidence?.required !== true
) failures.push("Forum Wave source is not bound to accepted Pages gate evidence");
if (
  wave.observed_run?.wave_admission?.format !== "forum_page_builder_wave_admission_v1" ||
  wave.observed_run?.wave_admission?.status !== "forum_wave_inputs_admitted_observed_control_plane_pending" ||
  wave.observed_run?.wave_admission?.source_status !== "source_ready_maintainer_execution_pending" ||
  wave.observed_run?.wave_admission?.execution_status !== "maintainer_execution_pending"
) failures.push("Forum Wave admission cursor drifted");
if (!(wave.observed_run?.required_evidence ?? []).includes("admission")) {
  failures.push("Forum Wave live evidence must require the admission section");
}
, marker, "Pages/Page Builder parity actualization");

if (contract.next_cursor?.forum_wave_admission !== "source_ready_maintainer_execution_pending") {
  failures.push("Wave admission next cursor drifted");
}
if (contract.next_cursor?.observed_control_plane_wave !== "blocked_on_admitted_exact_source_inputs") {
  failures.push("observed control-plane Wave cursor drifted");
}

if (failures.length) {
  console.error("[verify-forum-page-builder-wave-admission] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-forum-page-builder-wave-admission] PASS source_ready=true execution=pending wave=not_run");
