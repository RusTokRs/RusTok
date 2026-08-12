#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const failures = [];
const files = {
  contract: "crates/rustok-forum/contracts/evidence/forum-page-builder-wave-admission-runner-test-source.json",
  runner: "scripts/evidence/admit-forum-page-builder-wave.mjs",
  test: "scripts/evidence/admit-forum-page-builder-wave.test.mjs",
  verifier: "scripts/verify/verify-forum-page-builder-wave-admission-runner-tests.mjs",
  admissionGuard: "scripts/verify/verify-forum-page-builder-wave-admission.mjs",
  admissionSource: "crates/rustok-forum/contracts/evidence/forum-page-builder-wave-admission-source.json",
  actualization: "docs/modules/forum-page-builder-wave-admission-runner-tests-actualization-2026-08-12.md",
  workflow: ".github/workflows/pages-page-builder-provider-health.yml",
};
const abs = (value) => path.join(repoRoot, value);
const read = (value) => fs.readFileSync(abs(value), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const canonicalJson = (value) => {
  const normalize = (input) => {
    if (Array.isArray(input)) return input.map(normalize);
    if (input !== null && typeof input === "object") {
      return Object.fromEntries(
        Object.entries(input)
          .sort(([left], [right]) => left.localeCompare(right))
          .map(([key, nested]) => [key, normalize(nested)]),
      );
    }
    return input;
  };
  return JSON.stringify(normalize(value));
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
  console.error("[verify-forum-page-builder-wave-admission-runner-tests] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const contract = JSON.parse(read(files.contract));
const admissionSource = JSON.parse(read(files.admissionSource));
const runner = read(files.runner);
const test = read(files.test);
const admissionGuard = read(files.admissionGuard);
const actualization = read(files.actualization);
const workflow = read(files.workflow);

const expectedCases = [
  "valid_admission",
  "gate_not_accepted",
  "gate_promotion_overclaim",
  "browser_digest_mismatch",
  "browser_required_fact_missing",
  "runtime_command_argv_drift",
  "runtime_source_hash_tamper",
  "serverfn_live_commit_unverified",
  "serverfn_privacy_overclaim",
  "serverfn_cryptographic_binding_overclaim",
];

if (
  contract.format !== "forum_page_builder_wave_admission_runner_test_source_v1" ||
  contract.status !== "source_ready_synthetic_execution" ||
  contract.module !== "forum" ||
  contract.wave !== "1" ||
  contract.production_runner !== files.runner ||
  contract.test_runner !== files.test ||
  contract.source_verifier !== files.verifier
) {
  failures.push("runner-test source identity drifted");
}
if (canonicalJson(contract.synthetic_cases) !== canonicalJson(expectedCases)) {
  failures.push("synthetic case set drifted");
}
for (const [key, expected] of [
  ["production_runner_must_be_invoked_as_child_process", true],
  ["source_hashes_must_be_derived_from_current_checkout", true],
  ["pages_gate_must_be_accepted_and_non_promoting", true],
  ["browser_repo_digest_must_match_pages_gate", true],
  ["browser_required_facts_must_pass", true],
  ["runtime_commands_must_match_execution_contract", true],
  ["serverfn_live_source_commit_must_be_verified", true],
  ["serverfn_privacy_and_non_cryptographic_binding_boundaries_must_hold", true],
  ["successful_output_must_keep_observed_wave_pending", true],
  ["successful_output_must_keep_forum_wave_unaccepted", true],
]) {
  if (contract.fail_closed_requirements?.[key] !== expected) failures.push(`${key} drifted`);
}
for (const [key, expected] of [
  ["pages_gate_execution", "not_run_by_synthetic_test"],
  ["forum_browser_execution", "not_run_by_synthetic_test"],
  ["forum_runtime_authorization_execution", "not_run_by_synthetic_test"],
  ["forum_serverfn_deployment_attestation", "not_run_by_synthetic_test"],
  ["observed_control_plane_wave", "not_run_by_synthetic_test"],
  ["forum_wave_acceptance", "not_claimed"],
  ["ffa_fba_promotion", "not_claimed"],
]) {
  if (contract.live_execution_boundary?.[key] !== expected) failures.push(`${key} boundary drifted`);
}
for (const requiredPath of Object.values(files)) {
  if (requiredPath === files.admissionGuard || requiredPath === files.admissionSource) continue;
  if (!(contract.required_source_files ?? []).includes(requiredPath)) {
    failures.push(`runner-test required_source_files missing ${requiredPath}`);
  }
}

for (const marker of [
  'const runnerPath = path.join(repoRoot, "scripts/evidence/admit-forum-page-builder-wave.mjs")',
  'spawnSync("git", ["rev-parse", "HEAD"]',
  "function sourceHashes(contract)",
  "sourceHashes(gateContract)",
  "sourceHashes(browserContract)",
  "sourceHashes(runtimeContract)",
  "sourceHashes(serverfnContract)",
  "sourceHashes(admissionContract)",
  "gate.gate.accepted = false",
  "gate.boundaries.forum_wave_accepted = true",
  "browser.deployment_digest = OTHER_DEPLOYMENT_DIGEST",
  "browser.observations.full.facts.pages_save_completed = false",
  'runtime.commands[0].args = [...runtime.commands[0].args, "--synthetic-drift"]',
  'runtime.source_files[firstPath] = "0".repeat(64)',
  "serverfn.live_server_source_commit_verified_equal_checkout = false",
  "serverfn.privacy.credential_values_persisted = true",
  "serverfn.target.cryptographic_origin_to_repo_digest_binding = true",
  "output.admission.observed_control_plane_wave_pending",
  "output.boundaries.forum_wave_accepted",
  "Forum Page Builder Wave admission runner tests passed",
]) need(test, marker, "runner synthetic test");

for (const marker of [
  "fetch(",
  "@playwright/test",
  'spawnSync("cargo"',
  "updateModuleSettings",
  "forum_wave_accepted: true",
  "observed_control_plane_wave_executed: true",
]) forbid(test, marker, "runner synthetic test");

for (const marker of [
  'gate.accepted !== true',
  'validateCommandResults(document.commands, runtimeContract.commands',
  'live_server_source_commit_verified_equal_checkout',
  'target.cryptographic_origin_to_repo_digest_binding !== false',
  'observed_control_plane_wave_executed: false',
  'forum_wave_accepted: false',
]) need(runner, marker, "production admission runner");
need(admissionGuard, "PASS source_ready=true execution=pending wave=not_run", "admission source guard");

for (const marker of [
  "forum-wave-admission-runner-tests-source-ready",
  "production admission CLI",
  "synthetic packets are not live evidence",
  "observed control-plane Wave remains pending",
  "Forum Wave remains unaccepted",
]) need(actualization, marker, "runner-test actualization");

for (const marker of [
  "crates/rustok-forum/contracts/evidence/forum-page-builder-wave-admission-runner-test-source.json",
  "scripts/evidence/admit-forum-page-builder-wave.test.mjs",
  "scripts/verify/verify-forum-page-builder-wave-admission-runner-tests.mjs",
  "Verify Forum Wave admission source",
  "Verify Forum Wave admission runner test source",
  "Forum Wave admission runner synthetic tests",
  "node scripts/verify/verify-forum-page-builder-wave-admission.mjs",
  "node scripts/verify/verify-forum-page-builder-wave-admission-runner-tests.mjs",
  "node scripts/evidence/admit-forum-page-builder-wave.test.mjs",
]) need(workflow, marker, "focused workflow");

if (
  admissionSource.status !== "source_ready_maintainer_execution_pending" ||
  admissionSource.next_cursor?.observed_control_plane_wave !== "blocked_on_admitted_exact_source_inputs"
) {
  failures.push("Forum Wave admission/live cursor drifted");
}

if (failures.length) {
  console.error("[verify-forum-page-builder-wave-admission-runner-tests] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "[verify-forum-page-builder-wave-admission-runner-tests] PASS synthetic_runner_coverage=source_ready live_admission=pending observed_wave=blocked",
);
