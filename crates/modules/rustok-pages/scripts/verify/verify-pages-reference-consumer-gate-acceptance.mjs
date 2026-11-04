#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  contract: "crates/modules/rustok-pages/contracts/evidence/pages-reference-consumer-gate-acceptance-source.json",
  runner: "scripts/evidence/accept-pages-reference-consumer-gate.mjs",
  gate: "crates/modules/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json",
  candidateContract: "crates/modules/rustok-pages/contracts/evidence/pages-reference-consumer-gate-execution-contract.json",
  observedSource: "crates/modules/rustok-pages/contracts/evidence/pages-builder-provider-health-observed-acceptance-source.json",
  forumAdmission: "crates/modules/rustok-forum/contracts/evidence/forum-page-builder-wave-admission-source.json",
};
const abs = (value) => path.join(repoRoot, value);
const read = (value) => fs.readFileSync(abs(value), "utf8");
const need = (source, marker, label) => { if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`); };
const forbid = (source, marker, label) => { if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`); };

for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(abs(relativePath))) { failures.push(`${label}: missing ${relativePath}`); continue; }
  const stats = fs.lstatSync(abs(relativePath));
  if (!stats.isFile() || stats.isSymbolicLink()) failures.push(`${label}: must be a regular non-symlink file`);
}
if (failures.length) process.exit(1);

const contract = JSON.parse(read(files.contract));
const gate = JSON.parse(read(files.gate));
const candidateContract = JSON.parse(read(files.candidateContract));
const observedSource = JSON.parse(read(files.observedSource));
const forumAdmission = JSON.parse(read(files.forumAdmission));
const runner = read(files.runner);

if (contract.format !== "pages_reference_consumer_gate_acceptance_source_v1" || contract.status !== "source_ready_maintainer_execution_pending") failures.push("acceptance source identity drifted");
if (candidateContract.output?.format !== "pages_reference_consumer_gate_candidate_v1" || candidateContract.output?.status !== "component_execution_passed_owner_review_pending") failures.push("candidate predecessor drifted");
if (observedSource.format !== "pages_builder_provider_health_observed_acceptance_source_v1") failures.push("observed-health predecessor drifted");
if (gate.accepted !== false || gate.current_boundary?.execution_gate !== "pending" || gate.current_boundary?.provider_health !== "unobserved" || gate.current_boundary?.forum_wave_blocker !== "pages_reference_consumer_gate") failures.push("source gate must remain fail-closed before owner decision");
if (
  forumAdmission.format !== "forum_page_builder_wave_admission_source_v1" ||
  forumAdmission.status !== "source_ready_maintainer_execution_pending" ||
  forumAdmission.pages_gate_input?.format !== "pages_reference_consumer_gate_acceptance_v1" ||
  forumAdmission.pages_gate_input?.required_status !== "owner_accepted_pages_reference_consumer_gate" ||
  forumAdmission.output?.format !== "forum_page_builder_wave_admission_v1" ||
  forumAdmission.output?.status !== "forum_wave_inputs_admitted_observed_control_plane_pending"
) failures.push("Forum Wave admission successor drifted");

for (const [object, key, expected] of [
  [contract.candidate_input, "retained_source_hashes_must_match_contract_and_checkout", true],
  [contract.candidate_input, "retained_input_hash_records_must_be_bounded", true],
  [contract.candidate_input, "command_records_must_match_execution_contract_exactly", true],
  [contract.candidate_input, "provider_health_must_equal_unobserved", true],
  [contract.observed_health_input, "required_decision", "accept_observed_runtime_evidence"],
  [contract.observed_health_input, "source_commit_must_equal_checkout_head_and_candidate", true],
  [contract.observed_health_input, "deployment_image_digest_must_equal_candidate", true],
  [contract.observed_health_input, "deployment_id_must_be_bounded", true],
  [contract.observed_health_input, "eligible_for_pages_gate_review_must_be_true", true],
  [contract.observed_health_input, "current_provider_health_must_not_be_asserted", true],
  [contract.observed_health_input, "historical_health_lease_must_not_be_extended", true],
  [contract.owner_decision, "accepted_gate_requires_rollback_decision", "retain_reference_consumer_candidate"],
  [contract.owner_decision, "rejected_gate_requires_rollback_decision", "rollback_reference_consumer_candidate"],
  [contract.owner_decision, "decision_packet_does_not_execute_rollback", true],
  [contract.output, "accepted_status", "owner_accepted_pages_reference_consumer_gate"],
  [contract.source_gate_boundary, "source_gate_must_be_fail_closed_before_decision", true],
  [contract.source_gate_boundary, "pages_reference_consumer_gate_source_remains_accepted_false_until_maintainer_execution", true],
  [contract.downstream_forum_wave, "accepted_gate_packet_is_required_input", true],
  [contract.downstream_forum_wave, "accepted_gate_packet_does_not_accept_forum_wave", true],
  [contract.downstream_forum_wave, "observed_control_plane_wave_requires_separate_admission", true],
  [contract.non_claims, "owner_gate_decision_executed", false],
  [contract.non_claims, "pages_reference_consumer_gate_accepted_in_source", false],
  [contract.non_claims, "tests_run", false],
]) if (object?.[key] !== expected) failures.push(`${key} drifted`);

if (
  contract.downstream_forum_wave?.admission_source !== files.forumAdmission ||
  contract.downstream_forum_wave?.admission_output_format !== "forum_page_builder_wave_admission_v1" ||
  contract.downstream_forum_wave?.admission_output_status !== "forum_wave_inputs_admitted_observed_control_plane_pending"
) failures.push("Pages gate downstream Forum admission pointer drifted");

, marker, "acceptance actualization");
, marker, "parity actualization");

if (contract.next_cursor?.reference_consumer_gate_acceptance !== "source_ready_maintainer_execution_pending") failures.push("acceptance cursor drifted");
if (contract.next_cursor?.forum_wave_admission !== "source_ready_maintainer_execution_pending") failures.push("Forum Wave admission cursor drifted");
if (contract.next_cursor?.forum_observed_wave !== "blocked_on_admitted_exact_source_inputs") failures.push("Forum observed Wave cursor drifted");

if (failures.length) {
  console.error("[verify-pages-reference-consumer-gate-acceptance] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-reference-consumer-gate-acceptance] PASS source_ready=true execution=pending forum_wave_admission=source_ready");
