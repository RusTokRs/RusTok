#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  contract: "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-acceptance-source.json",
  runner: "scripts/evidence/accept-pages-reference-consumer-gate.mjs",
  gate: "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json",
  candidateContract: "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-execution-contract.json",
  observedSource: "crates/rustok-pages/contracts/evidence/pages-builder-provider-health-observed-acceptance-source.json",
  overlay: "docs/modules/pages-page-builder-reference-consumer-gate-acceptance-actualization-2026-08-10.md",
  parity: "docs/modules/pages-page-builder-plan-parity-actualization-2026-08-08.md",
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
const runner = read(files.runner);
const overlay = read(files.overlay);
const parity = read(files.parity);

if (contract.format !== "pages_reference_consumer_gate_acceptance_source_v1" || contract.status !== "source_ready_maintainer_execution_pending") failures.push("acceptance source identity drifted");
if (candidateContract.output?.format !== "pages_reference_consumer_gate_candidate_v1" || candidateContract.output?.status !== "component_execution_passed_owner_review_pending") failures.push("candidate predecessor drifted");
if (observedSource.format !== "pages_builder_provider_health_observed_acceptance_source_v1") failures.push("observed-health predecessor drifted");
if (gate.accepted !== false || gate.current_boundary?.execution_gate !== "pending" || gate.current_boundary?.provider_health !== "unobserved") failures.push("source gate must remain accepted=false, execution pending, rollout health unobserved");

for (const [object, key, expected] of [
  [contract.candidate_input, "retained_source_hashes_must_match_contract_and_checkout", true],
  [contract.candidate_input, "provider_health_must_equal_unobserved", true],
  [contract.observed_health_input, "required_decision", "accept_observed_runtime_evidence"],
  [contract.observed_health_input, "source_commit_must_equal_checkout_head_and_candidate", true],
  [contract.observed_health_input, "deployment_image_digest_must_equal_candidate", true],
  [contract.observed_health_input, "eligible_for_pages_gate_review_must_be_true", true],
  [contract.observed_health_input, "current_provider_health_must_not_be_asserted", true],
  [contract.observed_health_input, "historical_health_lease_must_not_be_extended", true],
  [contract.owner_decision, "accepted_gate_requires_rollback_decision", "retain_reference_consumer_candidate"],
  [contract.owner_decision, "rejected_gate_requires_rollback_decision", "rollback_reference_consumer_candidate"],
  [contract.owner_decision, "decision_packet_does_not_execute_rollback", true],
  [contract.output, "accepted_status", "owner_accepted_pages_reference_consumer_gate"],
  [contract.source_gate_boundary, "pages_reference_consumer_gate_source_remains_accepted_false_until_maintainer_execution", true],
  [contract.non_claims, "owner_gate_decision_executed", false],
  [contract.non_claims, "pages_reference_consumer_gate_accepted_in_source", false],
  [contract.non_claims, "tests_run", false],
]) if (object?.[key] !== expected) failures.push(`${key} drifted`);

for (const marker of [
  'const ACCEPT_DECISION = "accept_pages_reference_consumer_gate"',
  'const RETAIN_DECISION = "retain_reference_consumer_candidate"',
  'const ROLLBACK_DECISION = "rollback_reference_consumer_candidate"',
  'spawnSync("git", ["rev-parse", "HEAD"]',
  'verifyRetainedSourceHashes(document, candidateContract, "source_sha256", "reference candidate")',
  'verifyRetainedSourceHashes(document, observedSource, "source_files", "observed-health acceptance")',
  'candidate.provider_health !== "unobserved"',
  'decision.value !== OBSERVED_ACCEPT_DECISION',
  'digest !== candidate.deploymentDigest',
  'gate.eligible_for_pages_gate_review !== true',
  'observed.current_provider_health_asserted !== false',
  'binding.health_lease_extended !== false',
  'rollback_action_performed: false',
  'canonical_source_mutated: false',
  'forum_wave_accepted: false',
  'automatic_downstream_promotion: false',
  'source_files: sourceHashes(contract)',
]) need(runner, marker, "gate acceptance runner");
for (const marker of ["fetch(", "@playwright/test", "cargo test", "gate.accepted = true", "forum_wave_accepted: true", "rollback_action_performed: true"]) forbid(runner, marker, "gate acceptance runner");

for (const marker of [
  "reference-consumer-gate-acceptance-source-ready",
  "same exact source commit and immutable RepoDigest",
  "retain_reference_consumer_candidate",
  "rollback_reference_consumer_candidate",
  "does not assert current provider health",
  "Tests were not run",
]) need(overlay, marker, "acceptance actualization");
for (const marker of [
  "reference-consumer-gate-acceptance-source-ready",
  "pages-page-builder-reference-consumer-gate-acceptance-actualization-2026-08-10.md",
  "Pages reference-consumer gate acceptance [source-ready / maintainer execution pending]",
]) need(parity, marker, "parity actualization");

if (contract.next_cursor?.reference_consumer_gate_acceptance !== "source_ready_maintainer_execution_pending") failures.push("acceptance cursor drifted");
if (contract.next_cursor?.forum_observed_wave !== "blocked_on_accepted_pages_gate_packet") failures.push("Forum Wave cursor drifted");

if (failures.length) {
  console.error("[verify-pages-reference-consumer-gate-acceptance] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-reference-consumer-gate-acceptance] PASS source_ready=true execution=pending source_gate_accepted=false");
