#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const files = {
  contract: "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-execution-contract.json",
  evidence: "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-evidence-harness-source.json",
  gate: "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json",
  runner: "scripts/evidence/pages-reference-consumer-gate-evidence.mjs",
  artifact: "crates/rustok-pages/contracts/evidence/pages-inline-edit-artifact-http-execution-contract.json",
  browser: "crates/rustok-pages/contracts/evidence/pages-inline-edit-browser-execution-contract.json",
  matrix: "crates/rustok-pages/contracts/evidence/pages-builder-rollout-runtime-matrix-execution-contract.json",
  feature: "crates/rustok-pages/contracts/evidence/pages-builder-rollout-feature-preflight-execution-contract.json",
  matrixGuard: "crates/rustok-pages/scripts/verify/verify-pages-builder-rollout-runtime-matrix-harness.mjs",
  featureGuard: "crates/rustok-pages/scripts/verify/verify-pages-builder-rollout-feature-preflight-harness.mjs",
  packet: "docs/modules/pages-page-builder-reference-consumer-gate-evidence-harness-actualization-2026-08-08.md",
};
const failures = [];
const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
const need = (source, marker, label) => { if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`); };
const forbid = (source, marker, label) => { if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`); };

for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(absolute(relativePath))) {
    failures.push(`${label}: missing ${relativePath}`);
    continue;
  }
  const stats = fs.lstatSync(absolute(relativePath));
  if (!stats.isFile() || stats.isSymbolicLink()) failures.push(`${label}: ${relativePath} must be a regular non-symlink file`);
}
if (failures.length) {
  console.error("[verify-pages-reference-consumer-gate-evidence-harness] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const contract = JSON.parse(read(files.contract));
const evidence = JSON.parse(read(files.evidence));
const gate = JSON.parse(read(files.gate));
const artifact = JSON.parse(read(files.artifact));
const browser = JSON.parse(read(files.browser));
const matrix = JSON.parse(read(files.matrix));
const feature = JSON.parse(read(files.feature));
const runner = read(files.runner);
const packet = read(files.packet);

if (
  contract.schema_version !== 1 ||
  contract.module !== "pages" ||
  contract.packet !== "pages-reference-consumer-gate-candidate" ||
  contract.status !== "source_ready_maintainer_execution_pending"
) failures.push("candidate contract identity drifted");

const inputExpectations = [
  ["artifact_http", "RUSTOK_PAGES_REFERENCE_GATE_ARTIFACT_HTTP_EVIDENCE", artifact.output?.format, artifact.output?.status],
  ["browser", "RUSTOK_PAGES_REFERENCE_GATE_BROWSER_EVIDENCE", browser.output?.format, browser.output?.status],
  ["rollout_matrix", "RUSTOK_PAGES_REFERENCE_GATE_ROLLOUT_MATRIX_EVIDENCE", matrix.output?.format, matrix.output?.status],
  ["rollout_feature_preflight", "RUSTOK_PAGES_REFERENCE_GATE_ROLLOUT_FEATURE_PREFLIGHT_EVIDENCE", feature.output?.format, feature.output?.status],
];
for (const [key, environment, format, status] of inputExpectations) {
  const input = contract.inputs?.[key];
  if (input?.environment !== environment || input?.format !== format || input?.status !== status) {
    failures.push(`${key} input is not tied to its producer output contract`);
  }
}
for (const [key, expected] of Object.entries({
  browser_predecessor_hash_must_match: true,
  same_api_deployment_digest_required: true,
  api_origin_hash_must_match_browser: true,
  settings_restore_required: true,
  all_required_profiles_must_pass: true,
  provider_health_must_remain_unobserved: true,
})) {
  if (contract.inputs?.rollout_matrix?.[key] !== expected) failures.push(`rollout_matrix.${key} must be ${expected}`);
}
for (const [key, expected] of Object.entries({
  browser_predecessor_hash_must_match: true,
  rollout_matrix_predecessor_hash_must_match: true,
  same_api_deployment_digest_required: true,
  api_origin_hash_must_match_browser: true,
  settings_restore_required: true,
  all_required_profiles_must_match_feature_disabled_catalog: true,
})) {
  if (contract.inputs?.rollout_feature_preflight?.[key] !== expected) failures.push(`rollout_feature_preflight.${key} must be ${expected}`);
}

const guards = new Map((contract.source_guards ?? []).map((entry) => [entry.id, entry]));
for (const [id, script] of [
  ["rollout_matrix_harness_source", files.matrixGuard],
  ["rollout_feature_preflight_harness_source", files.featureGuard],
  ["reference_gate_harness_source", "crates/rustok-pages/scripts/verify/verify-pages-reference-consumer-gate-evidence-harness.mjs"],
]) {
  const guard = guards.get(id);
  if (guard?.program !== "node" || JSON.stringify(guard?.args) !== JSON.stringify([script])) failures.push(`${id} guard drifted`);
  if (!gate.gate?.required_source_guards?.includes(script)) failures.push(`canonical gate is missing ${script}`);
}
for (const required of gate.gate?.required_source_guards ?? []) {
  if (![...guards.values()].some((guard) => guard.program === "node" && guard.args?.[0] === required)) {
    failures.push(`candidate source guards are missing canonical gate guard ${required}`);
  }
}

for (const [key, expected] of Object.entries({
  rollout_matrix_same_source_commit: true,
  rollout_matrix_must_bind_exact_browser_hash: true,
  rollout_matrix_settings_restore_must_be_verified: true,
  rollout_feature_preflight_same_source_commit: true,
  rollout_feature_preflight_must_bind_exact_browser_hash: true,
  rollout_feature_preflight_must_bind_exact_matrix_hash: true,
  rollout_feature_preflight_settings_restore_must_be_verified: true,
  canonical_feature_disabled_catalog_must_pass: true,
  browser_intent_denial_remains_separate_from_feature_disabled_catalog: true,
})) {
  if (contract.candidate_requirements?.[key] !== expected) failures.push(`candidate_requirements.${key} must be ${expected}`);
}
if (
  contract.output?.format !== "pages_reference_consumer_gate_candidate_v1" ||
  contract.output?.status !== "component_execution_passed_owner_review_pending" ||
  contract.output?.automatic_gate_acceptance !== false ||
  contract.output?.automatic_source_mutation !== false ||
  contract.output?.automatic_ffa_fba_promotion !== false
) failures.push("candidate output boundary drifted");

for (const relativePath of [
  files.matrix,
  files.feature,
  files.matrixGuard,
  files.featureGuard,
  "crates/rustok-pages/contracts/evidence/pages-builder-rollout-feature-preflight-harness-source.json",
  "apps/next-admin/playwright.pages-builder-rollout-feature-preflight.config.ts",
  "apps/next-admin/tests/pages-builder-rollout-feature-preflight/feature-preflight.spec.ts",
  "crates/rustok-pages/src/graphql/builder_rollout.rs",
]) {
  if (!contract.required_source_files?.includes(relativePath)) failures.push(`candidate required_source_files is missing ${relativePath}`);
}

if (evidence.format !== "pages_reference_consumer_gate_evidence_harness_source_v1") failures.push("source evidence format drifted");
if (evidence.status !== "pages_reference_consumer_gate_evidence_harness_source_unvalidated") failures.push("source evidence status drifted");
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) failures.push("source evidence execution must remain empty");
for (const [key, value] of Object.entries(evidence.validation ?? {})) if (value !== false) failures.push(`validation.${key} must remain false`);
for (const key of [
  "artifact_http_packet_is_required",
  "browser_packet_is_required",
  "rollout_matrix_packet_is_required",
  "rollout_feature_preflight_packet_is_required",
  "rollout_matrix_must_bind_exact_browser_hash",
  "rollout_matrix_settings_restore_must_be_verified",
  "rollout_feature_preflight_must_bind_exact_browser_hash",
  "rollout_feature_preflight_must_bind_exact_matrix_hash",
  "rollout_feature_preflight_settings_restore_must_be_verified",
  "rollout_feature_preflight_all_four_profiles_are_rechecked",
  "rollout_feature_preflight_requires_feature_disabled_kind",
  "rollout_feature_preflight_requires_FEATURE_DISABLED_code",
  "browser_intent_FLY_CAPABILITY_DENIED_is_not_used_as_FEATURE_DISABLED_evidence",
  "rollout_feature_preflight_source_guard_is_required_by_gate",
  "raw_input_packets_are_not_persisted",
  "raw_rollout_settings_are_not_persisted",
  "candidate_output_is_atomic",
  "candidate_status_is_owner_review_pending",
  "provider_health_remains_unobserved",
  "owner_signoff_remains_pending",
  "rollback_decision_remains_pending",
  "gate_acceptance_is_not_automatic",
]) if (evidence.source_contract?.[key] !== true) failures.push(`source_contract.${key} must be true`);
for (const key of [
  "tests_run",
  "source_verifiers_run",
  "cargo_run",
  "node_run",
  "artifact_http_input_observed",
  "browser_input_observed",
  "rollout_matrix_input_observed",
  "rollout_feature_preflight_input_observed",
  "candidate_packet_produced",
  "owner_review_observed",
  "gate_accepted",
  "forum_wave_accepted",
  "ffa_promoted",
  "fba_promoted",
]) if (evidence.source_contract?.[key] !== false) failures.push(`source_contract.${key} must remain false`);

if (
  gate.accepted !== false ||
  gate.current_boundary?.execution_gate !== "pending" ||
  gate.current_boundary?.provider_health !== "unobserved" ||
  gate.current_boundary?.reference_candidate_rollout_matrix_input !== "source_ready_required_before_candidate" ||
  gate.current_boundary?.reference_candidate_feature_preflight_input !== "source_ready_required_before_candidate"
) failures.push("canonical gate pending boundary drifted");
if (
  gate.execution_harness?.required_inputs?.rollout_matrix !== "pages_builder_rollout_runtime_matrix_v1" ||
  gate.execution_harness?.required_inputs?.rollout_feature_preflight !== "pages_builder_rollout_feature_preflight_v1" ||
  gate.execution_harness?.canonical_feature_disabled_code_required !== "FEATURE_DISABLED"
) failures.push("canonical gate execution-harness input registration drifted");

for (const marker of [
  "contract.inputs.rollout_feature_preflight.environment",
  "validateRolloutFeaturePreflight(",
  "matrix.sha256 !== matrixInput.record.sha256",
  'result.error_kind !== "feature-disabled"',
  'result.error_code !== "FEATURE_DISABLED"',
  "canonical_feature_disabled_catalog_passed: true",
  "browser_intent_denial_kept_separate: true",
  "provider_health: \"unobserved\"",
  "owner_signoff: \"pending\"",
  "rollback_decision: \"pending\"",
  "gate_acceptance: \"pending\"",
  "raw_input_packets_persisted: false",
  "raw_rollout_settings_persisted: false",
  "shell: false",
]) need(runner, marker, "candidate runner");
for (const marker of [
  "shell: true",
  "gate_accepted: true",
  "provider_health: \"healthy\"",
  "ffa_promoted: true",
  "fba_promoted: true",
]) forbid(runner, marker, "candidate runner");

for (const marker of [
  "Four required machine packets",
  "FLY_CAPABILITY_DENIED",
  "FEATURE_DISABLED",
  "RUSTOK_PAGES_REFERENCE_GATE_ROLLOUT_FEATURE_PREFLIGHT_EVIDENCE",
  "candidate owner review remains pending",
  "does not accept `pages_reference_consumer_gate`",
  "does not claim provider health",
  "does not promote FFA/FBA",
  "No tests, verifiers, Cargo commands, Node commands",
]) need(packet, marker, "candidate actualization");

if (failures.length) {
  console.error("[verify-pages-reference-consumer-gate-evidence-harness] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-reference-consumer-gate-evidence-harness] PASS");
