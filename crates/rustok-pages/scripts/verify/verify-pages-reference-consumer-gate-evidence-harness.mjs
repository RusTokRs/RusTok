#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..", "..", "..");
const failures = [];
const files = {
  contract:
    "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-execution-contract.json",
  evidence:
    "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-evidence-harness-source.json",
  gate: "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json",
  runner: "scripts/evidence/pages-reference-consumer-gate-evidence.mjs",
  artifactHttpContract:
    "crates/rustok-pages/contracts/evidence/pages-inline-edit-artifact-http-execution-contract.json",
  browserContract:
    "crates/rustok-pages/contracts/evidence/pages-inline-edit-browser-execution-contract.json",
  rolloutMatrixContract:
    "crates/rustok-pages/contracts/evidence/pages-builder-rollout-runtime-matrix-execution-contract.json",
  rolloutMatrixSourceVerifier:
    "crates/rustok-pages/scripts/verify/verify-pages-builder-rollout-runtime-matrix-harness.mjs",
  planParity:
    "crates/rustok-page-builder/scripts/verify/verify-pages-page-builder-plan-parity.mjs",
  packet:
    "docs/modules/pages-page-builder-reference-consumer-gate-evidence-harness-actualization-2026-08-08.md",
};

const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const exact = (actual, expected, label) => {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) failures.push(`${label} drifted`);
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
if (failures.length > 0) {
  console.error("[verify-pages-reference-consumer-gate-evidence-harness] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const contract = JSON.parse(read(files.contract));
const evidence = JSON.parse(read(files.evidence));
const gate = JSON.parse(read(files.gate));
const artifactHttpContract = JSON.parse(read(files.artifactHttpContract));
const browserContract = JSON.parse(read(files.browserContract));
const rolloutMatrixContract = JSON.parse(read(files.rolloutMatrixContract));
const runner = read(files.runner);
const packet = read(files.packet);

if (
  contract.schema_version !== 1 ||
  contract.module !== "pages" ||
  contract.packet !== "pages-reference-consumer-gate-candidate" ||
  contract.status !== "source_ready_maintainer_execution_pending"
) {
  failures.push("execution contract identity drifted");
}
if (contract.runner !== files.runner || contract.source_gate !== files.gate) {
  failures.push("execution contract runner/source-gate paths drifted");
}
exact(
  contract.inputs?.artifact_http,
  {
    environment: "RUSTOK_PAGES_REFERENCE_GATE_ARTIFACT_HTTP_EVIDENCE",
    format: "pages_inline_edit_artifact_http_execution_v1",
    status: "artifact_http_execution_passed_browser_rollout_pending",
    same_source_commit_required: true,
    deployment_digest_required: true,
  },
  "artifact/HTTP gate input",
);
exact(
  contract.inputs?.browser,
  {
    environment: "RUSTOK_PAGES_REFERENCE_GATE_BROWSER_EVIDENCE",
    format: "pages_inline_edit_browser_execution_v1",
    status: "browser_execution_passed_rollout_pending",
    same_source_commit_required: true,
    same_deployment_digest_required: true,
    artifact_http_hash_must_match: true,
  },
  "browser gate input",
);
exact(
  contract.inputs?.rollout_matrix,
  {
    environment: "RUSTOK_PAGES_REFERENCE_GATE_ROLLOUT_MATRIX_EVIDENCE",
    format: "pages_builder_rollout_runtime_matrix_v1",
    status: "four_profile_runtime_matrix_passed_owner_review_pending",
    same_source_commit_required: true,
    browser_predecessor_hash_must_match: true,
    same_api_deployment_digest_required: true,
    api_origin_hash_must_match_browser: true,
    admin_origin_hash_must_match_browser: true,
    settings_restore_required: true,
    all_required_profiles_must_pass: true,
    provider_health_must_remain_unobserved: true,
  },
  "rollout matrix gate input",
);
if (
  contract.inputs.artifact_http.format !== artifactHttpContract.output?.format ||
  contract.inputs.artifact_http.status !== artifactHttpContract.output?.status
) {
  failures.push("gate artifact/HTTP input is not tied to artifact/HTTP output contract");
}
if (
  contract.inputs.browser.format !== browserContract.output?.format ||
  contract.inputs.browser.status !== browserContract.output?.status
) {
  failures.push("gate browser input is not tied to browser output contract");
}
if (
  contract.inputs.rollout_matrix.format !== rolloutMatrixContract.output?.format ||
  contract.inputs.rollout_matrix.status !== rolloutMatrixContract.output?.status
) {
  failures.push("gate rollout-matrix input is not tied to rollout-matrix output contract");
}

const expectedSourceGuards = [
  ["plan_parity", "node", ["crates/rustok-page-builder/scripts/verify/verify-pages-page-builder-plan-parity.mjs"]],
  ["consumer_readiness", "node", ["crates/rustok-page-builder/scripts/verify/verify-page-builder-consumer-readiness.mjs", "pages"]],
  ["provider_status_source", "node", ["crates/rustok-page-builder/scripts/verify/verify-page-builder-admin-provider-status.mjs"]],
  ["metadata_properties_source", "node", ["crates/rustok-pages/scripts/verify/verify-pages-metadata-properties.mjs"]],
  ["metadata_revision_source", "node", ["crates/rustok-pages/scripts/verify/verify-pages-metadata-revision-isolation.mjs"]],
  ["inline_consumer_source", "node", ["crates/rustok-pages/scripts/verify/verify-pages-authenticated-inline-consumer.mjs"]],
  ["authoring_route_source", "node", ["crates/rustok-pages/scripts/verify/verify-pages-authenticated-authoring-route.mjs"]],
  ["asset_delivery_source", "node", ["crates/rustok-pages/scripts/verify/verify-pages-inline-edit-asset-delivery.mjs"]],
  ["admin_launch_source", "node", ["crates/rustok-pages/scripts/verify/verify-pages-inline-edit-admin-launch.mjs"]],
  ["release_composition_source", "node", ["crates/rustok-pages/scripts/verify/verify-pages-inline-edit-release-composition.mjs"]],
  ["anonymous_graph_source", "node", ["crates/rustok-pages/scripts/verify/verify-pages-anonymous-storefront-graph.mjs"]],
  ["cache_invalidation_source", "node", ["crates/rustok-pages/scripts/verify/verify-pages-cache-invalidation.mjs"]],
  ["artifact_rollback_source", "node", ["crates/rustok-pages/scripts/verify/verify-pages-artifact-rollback.mjs"]],
  ["rollout_matrix_harness_source", "node", [files.rolloutMatrixSourceVerifier]],
  ["reference_gate_harness_source", "node", ["crates/rustok-pages/scripts/verify/verify-pages-reference-consumer-gate-evidence-harness.mjs"]],
];
exact(
  contract.source_guards?.map(({ id, program, args }) => [id, program, args]),
  expectedSourceGuards,
  "source guard command set",
);

const expectedFocusedTests = [
  ["metadata_stale_revision", "cargo", ["test", "-p", "rustok-pages-admin", "stale_metadata_revision_short_circuits_before_patch_transport"]],
  ["metadata_dirty_fly_isolation", "cargo", ["test", "-p", "rustok-pages-admin", "metadata_save_is_document_free_and_preserves_dirty_fly_state"]],
  ["publish_sanitization", "cargo", ["test", "-p", "rustok-page-builder", "publish_sanitization::tests"]],
  ["static_publish_resource_limits", "cargo", ["test", "-p", "rustok-page-builder", "static_publish_resource_limits::tests"]],
  ["publish_rollback_cache_correlation", "cargo", ["test", "-p", "rustok-pages", "--test", "publish_rollback_cache_correlation"]],
  ["provider_degraded_profiles", "cargo", ["test", "-p", "rustok-page-builder-admin", "provider_status::tests"]],
];
exact(
  contract.focused_tests?.map(({ id, program, args }) => [id, program, args]),
  expectedFocusedTests,
  "focused test command set",
);

exact(
  contract.output,
  {
    environment: "RUSTOK_PAGES_REFERENCE_GATE_OUTPUT",
    default_path: "target/pages-reference-consumer-gate-candidate.json",
    format: "pages_reference_consumer_gate_candidate_v1",
    status: "component_execution_passed_owner_review_pending",
    atomic_replace: true,
    automatic_source_mutation: false,
    automatic_gate_acceptance: false,
    automatic_ffa_fba_promotion: false,
  },
  "candidate output contract",
);
exact(
  contract.candidate_requirements,
  {
    all_source_guards_must_pass: true,
    all_focused_tests_must_pass: true,
    artifact_http_and_browser_same_source_commit: true,
    artifact_http_and_browser_same_deployment_digest: true,
    browser_must_bind_exact_artifact_http_hash: true,
    rollout_matrix_same_source_commit: true,
    rollout_matrix_must_bind_exact_browser_hash: true,
    rollout_matrix_api_deployment_digest_must_match: true,
    rollout_matrix_origin_hashes_must_match_browser: true,
    rollout_matrix_settings_restore_must_be_verified: true,
    rollout_matrix_all_required_profiles_must_pass: true,
    provider_health_claim: "unobserved",
    owner_signoff: "pending_after_candidate",
    rollback_decision: "pending_after_candidate",
    gate_acceptance: "pending_after_candidate",
  },
  "candidate requirements",
);

for (const relativePath of Object.values(files)) {
  if (!contract.required_source_files?.includes(relativePath)) {
    failures.push(`required_source_files is missing ${relativePath}`);
  }
}
for (const required of [
  "crates/rustok-pages/contracts/evidence/pages-builder-rollout-runtime-matrix-harness-source.json",
  "apps/next-admin/playwright.pages-builder-rollout-matrix.config.ts",
  "apps/next-admin/tests/pages-builder-rollout-matrix/runtime-matrix.spec.ts",
  "crates/rustok-page-builder/src/publish_sanitization.rs",
  "crates/rustok-page-builder/src/static_publish_resource_limits.rs",
  "crates/rustok-page-builder/admin/src/provider_status.rs",
  "crates/rustok-pages/admin/src/metadata_properties.rs",
  "crates/rustok-pages/tests/publish_rollback_cache_correlation.rs",
]) {
  if (!contract.required_source_files?.includes(required)) {
    failures.push(`required_source_files is missing ${required}`);
  }
}
for (const forbiddenValue of [
  "tenant ids",
  "actor ids",
  "authorization headers",
  "cookies",
  "session ids",
  "grants",
  "authorization proofs",
  "signing keys",
  "database urls",
  "raw HTML",
  "raw request or response bodies",
  "raw stdout or stderr",
  "raw monitoring logs",
  "raw rollout settings",
]) {
  if (!contract.forbidden_retained_data?.includes(forbiddenValue)) {
    failures.push(`privacy boundary is missing ${forbiddenValue}`);
  }
}

if (evidence.format !== "pages_reference_consumer_gate_evidence_harness_source_v1") {
  failures.push("source evidence format drifted");
}
if (evidence.status !== "pages_reference_consumer_gate_evidence_harness_source_unvalidated") {
  failures.push("source evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("source evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`source evidence validation.${key} must remain false`);
}
for (const key of [
  "machine_execution_contract_added",
  "bounded_runner_added",
  "runner_uses_shell_false",
  "runner_allows_only_node_source_guards_and_cargo_tests",
  "runner_executes_only_contract_declared_commands",
  "runner_hashes_required_source_files",
  "runner_requires_exact_git_head",
  "artifact_http_packet_is_required",
  "browser_packet_is_required",
  "rollout_matrix_packet_is_required",
  "artifact_http_and_browser_source_commit_must_match_head",
  "artifact_http_and_browser_deployment_digest_must_match",
  "browser_packet_must_bind_exact_artifact_http_hash",
  "rollout_matrix_source_commit_must_match_head",
  "rollout_matrix_must_bind_exact_browser_hash",
  "rollout_matrix_api_deployment_digest_must_match",
  "rollout_matrix_api_origin_hash_must_match_browser",
  "rollout_matrix_admin_origin_hash_must_match_browser",
  "rollout_matrix_settings_restore_must_be_verified",
  "rollout_matrix_all_four_profiles_are_rechecked",
  "rollout_matrix_pages_owned_reads_are_rechecked",
  "rollout_matrix_ui_ssr_and_browser_intent_outcomes_are_rechecked",
  "rollout_matrix_provider_health_must_remain_unobserved",
  "rollout_matrix_source_guard_is_required_by_gate",
  "anonymous_authoring_exclusion_is_inherited_from_artifact_http_packet",
  "browser_save_replay_stale_and_expiry_evidence_is_rechecked",
  "metadata_revision_and_dirty_fly_tests_are_declared",
  "sanitization_and_resource_limit_tests_are_declared",
  "publish_rollback_cache_correlation_test_is_declared",
  "provider_degraded_profile_tests_are_declared",
  "required_gate_source_guards_are_declared",
  "raw_stdout_and_stderr_are_not_persisted",
  "raw_input_packets_are_not_persisted",
  "raw_rollout_settings_are_not_persisted",
  "candidate_output_is_atomic",
  "candidate_output_remains_inside_target",
  "candidate_status_is_owner_review_pending",
  "provider_health_remains_unobserved",
  "owner_signoff_remains_pending",
  "rollback_decision_remains_pending",
  "gate_acceptance_is_not_automatic",
  "canonical_source_is_not_mutated_automatically",
  "forum_wave_is_not_accepted_automatically",
  "ffa_is_not_promoted_automatically",
  "fba_is_not_promoted_automatically",
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`source_contract.${key} must be true`);
  }
}
for (const key of [
  "tests_run",
  "source_verifiers_run",
  "cargo_run",
  "node_run",
  "artifact_http_input_observed",
  "browser_input_observed",
  "rollout_matrix_input_observed",
  "candidate_packet_produced",
  "owner_review_observed",
  "gate_accepted",
  "forum_wave_accepted",
  "ffa_promoted",
  "fba_promoted",
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`source_contract.${key} must remain false`);
  }
}

if (
  gate.artifact !== "pages_reference_consumer_gate_source" ||
  gate.mode !== "source_ready" ||
  gate.accepted !== false ||
  gate.current_boundary?.execution_gate !== "pending" ||
  gate.current_boundary?.provider_health !== "unobserved"
) {
  failures.push("Pages source gate must remain source-ready, unaccepted, pending and unobserved");
}
if (gate.execution_harness?.status !== "source_ready_maintainer_execution_pending") {
  failures.push("Pages source gate execution_harness status drifted");
}
if (
  gate.execution_harness?.contract !== files.contract ||
  gate.execution_harness?.runner !== files.runner ||
  gate.execution_harness?.source_verifier !==
    "crates/rustok-pages/scripts/verify/verify-pages-reference-consumer-gate-evidence-harness.mjs" ||
  gate.execution_harness?.candidate_status !== "component_execution_passed_owner_review_pending" ||
  gate.execution_harness?.required_inputs?.rollout_matrix !==
    "pages_builder_rollout_runtime_matrix_v1" ||
  gate.execution_harness?.rollout_matrix_status_required !==
    "four_profile_runtime_matrix_passed_owner_review_pending" ||
  gate.execution_harness?.rollout_matrix_browser_predecessor_hash_required !== true ||
  gate.execution_harness?.rollout_matrix_settings_restore_required !== true
) {
  failures.push("Pages source gate execution_harness registration drifted");
}
for (const requiredGuard of [
  files.rolloutMatrixSourceVerifier,
  "crates/rustok-pages/scripts/verify/verify-pages-reference-consumer-gate-evidence-harness.mjs",
]) {
  if (!gate.gate?.required_source_guards?.includes(requiredGuard)) {
    failures.push(`Pages source gate must require ${requiredGuard}`);
  }
}
if (
  gate.current_boundary?.reference_candidate_rollout_matrix_input !==
  "source_ready_required_before_candidate"
) {
  failures.push("Pages source gate matrix-input cursor drifted");
}

for (const marker of [
  "spawnSync(",
  "shell: false",
  "contract.source_guards",
  "contract.focused_tests",
  "contract.inputs.rollout_matrix",
  "currentCommit()",
  "artifactHttp.sha256 !== artifactInput.record.sha256",
  "validateRolloutMatrix(",
  "predecessor.sha256 !== browserInput.record.sha256",
  "target.deployment_image_digest !== deploymentDigest",
  "originalSettings.restore_verified !== true",
  "validateRolloutProfile(profileId, profiles[profileId])",
  "rollout_matrix_browser_chain_bound: true",
  "rollout_matrix_profiles_passed: true",
  "rollout_matrix_settings_restored: true",
  "provider_health: \"unobserved\"",
  "owner_signoff: \"pending\"",
  "rollback_decision: \"pending\"",
  "gate_acceptance: \"pending\"",
  "canonical_source_mutated: false",
  "gate_accepted: false",
  "forum_wave_accepted: false",
  "raw_command_output_persisted: false",
  "raw_rollout_settings_persisted: false",
  "renameSync(temporary, location)",
]) {
  need(runner, marker, "candidate runner");
}
for (const marker of [
  "execSync(",
  "exec(",
  "shell: true",
  "accepted: true",
  "provider_health: \"healthy\"",
  "ffa_promoted: true",
  "fba_promoted: true",
]) {
  forbid(runner, marker, "candidate runner");
}

for (const marker of [
  "three existing machine packets",
  "RUSTOK_PAGES_REFERENCE_GATE_ROLLOUT_MATRIX_EVIDENCE",
  "candidate owner review remains pending",
  "does not accept `pages_reference_consumer_gate`",
  "does not claim provider health",
  "does not promote FFA/FBA",
  "No tests, verifiers, Cargo commands, Node commands, browser runs, HTTP requests, workflows or CI were run",
]) {
  need(packet, marker, "actualization packet");
}

if (failures.length > 0) {
  console.error("[verify-pages-reference-consumer-gate-evidence-harness] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

console.log("[verify-pages-reference-consumer-gate-evidence-harness] PASS");
