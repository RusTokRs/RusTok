#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const failures = [];
const files = {
  contract:
    "crates/rustok-page-builder/contracts/evidence/page-builder-static-sanitization-registry-admission-source.json",
  runner: "scripts/evidence/admit-page-builder-static-sanitization-registry-update.mjs",
  tests: "scripts/evidence/admit-page-builder-static-sanitization-registry-update.test.mjs",
  verifier: "scripts/verify/verify-page-builder-static-sanitization-registry-admission.mjs",
  executionSource:
    "crates/rustok-page-builder/contracts/evidence/page-builder-static-sanitization-execution-source.json",
  registry: "crates/rustok-page-builder/contracts/page-builder-fba-registry.json",
};

function absolute(relativePath) {
  return path.join(repoRoot, relativePath);
}

function read(relativePath) {
  try {
    const location = absolute(relativePath);
    const stat = fs.lstatSync(location);
    if (!stat.isFile() || stat.isSymbolicLink()) {
      failures.push(`${relativePath}: must be a regular non-symlink file`);
      return "";
    }
    return fs.readFileSync(location, "utf8");
  } catch (error) {
    failures.push(`${relativePath}: ${error.message}`);
    return "";
  }
}

function json(relativePath) {
  const source = read(relativePath);
  try {
    const document = JSON.parse(source);
    if (document === null || typeof document !== "object" || Array.isArray(document)) {
      failures.push(`${relativePath}: JSON root must be an object`);
      return {};
    }
    return document;
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return {};
  }
}

function requireValue(condition, message) {
  if (!condition) failures.push(message);
}

function requireText(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing '${marker}'`);
}

function forbidText(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden '${marker}'`);
}

function pointerValue(document, pointer) {
  if (typeof pointer !== "string" || !pointer.startsWith("/")) return undefined;
  let current = document;
  for (const rawToken of pointer.slice(1).split("/")) {
    const token = rawToken.replaceAll("~1", "/").replaceAll("~0", "~");
    if (current === null || typeof current !== "object" || !Object.hasOwn(current, token)) {
      return undefined;
    }
    current = current[token];
  }
  return current;
}

const contract = json(files.contract);
const executionSource = json(files.executionSource);
const registry = json(files.registry);
const runner = read(files.runner);
const tests = read(files.tests);

requireValue(
  contract.format === "page_builder_static_sanitization_registry_admission_source" &&
    contract.status === "source_ready_execution_receipt_pending" &&
    contract.scope === "page_builder_fba_static_sanitization",
  `${files.contract}: identity drifted`,
);
requireValue(
  contract.predecessor?.source_contract === files.executionSource &&
    contract.predecessor?.source_format === "page_builder_static_sanitization_execution_source_v1" &&
    contract.predecessor?.source_status === "source_ready_maintainer_execution_pending" &&
    contract.predecessor?.receipt_format === "page_builder_static_sanitization_execution_v1" &&
    contract.predecessor?.receipt_status ===
      "static_sanitization_execution_passed_registry_update_pending" &&
    contract.predecessor?.maximum_receipt_bytes === 1048576 &&
    contract.predecessor?.maximum_run_metadata_bytes === 2097152,
  `${files.contract}: predecessor boundary drifted`,
);
requireValue(
  executionSource.format === contract.predecessor?.source_format &&
    executionSource.status === contract.predecessor?.source_status &&
    executionSource.output?.format === contract.predecessor?.receipt_format &&
    executionSource.output?.success_status === contract.predecessor?.receipt_status,
  `${files.executionSource}: execution source identity drifted`,
);

requireValue(
  contract.github_run?.repository === "RusTokRs/RusTok" &&
    contract.github_run?.workflow_name === "Page Builder Static Sanitization Evidence" &&
    contract.github_run?.workflow_path ===
      ".github/workflows/page-builder-static-sanitization-evidence.yml" &&
    contract.github_run?.event === "push" &&
    contract.github_run?.head_branch === "main" &&
    contract.github_run?.required_status === "completed" &&
    contract.github_run?.required_conclusion === "success" &&
    contract.github_run?.run_id_must_equal_receipt === true &&
    contract.github_run?.run_attempt_must_equal_receipt === true &&
    contract.github_run?.head_sha_must_equal_receipt_source_commit === true &&
    contract.github_run?.saved_run_metadata_is_not_cryptographic_attestation === true &&
    contract.github_run?.maintainer_must_review_run_on_github === true,
  `${files.contract}: GitHub run admission boundary drifted`,
);

for (const key of [
  "receipt_source_commit_must_exist",
  "receipt_source_commit_must_be_ancestor_of_checkout_head",
  "receipt_source_sha256_must_cover_exact_execution_required_source_files",
  "receipt_source_sha256_must_match_files_at_executed_commit",
  "current_execution_required_source_files_must_match_executed_commit",
  "fba_registry_must_still_match_executed_before_state",
  "source_drift_requires_new_execution",
]) {
  requireValue(contract.source_lineage?.[key] === true, `${files.contract}: source_lineage.${key} must be true`);
}

requireValue(
  contract.target?.fba_registry === files.registry &&
    contract.target?.registry_required_status === "boundary_ready" &&
    contract.target?.executed_evidence_json_pointer ===
      "/provider/static_sanitization_contract/executed_evidence" &&
    contract.target?.required_before_value === "pending" &&
    contract.target?.admitted_after_value === "verified" &&
    contract.target?.registry_mutation_by_admission === false,
  `${files.contract}: target boundary drifted`,
);
requireValue(registry.status === "boundary_ready", `${files.registry}: status must remain boundary_ready`);
requireValue(
  pointerValue(registry, contract.target?.executed_evidence_json_pointer) === "pending",
  `${files.registry}: target executed_evidence must remain pending before admission execution`,
);

requireValue(
  contract.admission?.runner === files.runner &&
    contract.admission?.runner_test === files.tests &&
    contract.admission?.source_verifier === files.verifier &&
    contract.admission?.network_requests === false &&
    contract.admission?.github_api_requests === false &&
    contract.admission?.browser_execution === false &&
    contract.admission?.cargo_execution === false &&
    contract.admission?.database_access === false &&
    contract.admission?.registry_mutation === false &&
    contract.admission?.git_ref_mutation === false,
  `${files.contract}: admission execution boundary drifted`,
);
requireValue(
  contract.output?.format === "page_builder_static_sanitization_registry_admission" &&
    contract.output?.success_status ===
      "static_sanitization_execution_admitted_registry_update_pending" &&
    contract.output?.default_path ===
      "target/page-builder-static-sanitization-registry-admission.json" &&
    contract.output?.receipt_sha256_retained === true &&
    contract.output?.run_metadata_sha256_retained === true &&
    contract.output?.execution_source_commit_retained === true &&
    contract.output?.execution_source_sha256_retained === true &&
    contract.output?.run_identity_retained === true &&
    contract.output?.target_pointer_retained === true &&
    contract.output?.raw_receipt_path_retained === false &&
    contract.output?.raw_run_metadata_path_retained === false &&
    contract.output?.raw_logs_retained === false,
  `${files.contract}: output boundary drifted`,
);

for (const key of [
  "admission_is_not_registry_update",
  "admission_is_not_terminal_inventory_completion",
  "admission_is_not_owner_approval",
  "admission_is_not_platform_approval",
  "admission_does_not_promote_fba",
  "registry_change_requires_separate_evidence_containing_pr",
  "registry_change_must_bind_exact_admission_packet",
  "registry_change_must_change_only_the_admitted_node_for_this_evidence",
  "terminal_inventory_must_be_recomputed_after_registry_change",
]) {
  requireValue(contract.governance_boundary?.[key] === true, `${files.contract}: ${key} must be true`);
}
requireValue(
  contract.governance_boundary?.cryptographic_ci_attestation_claimed === false,
  `${files.contract}: cryptographic CI attestation must remain unclaimed`,
);
for (const [key, value] of Object.entries(contract.non_claims ?? {})) {
  requireValue(value === false, `${files.contract}: non_claims.${key} must remain false`);
}

for (const marker of [
  "--receipt",
  "--workflow-run",
  "evaluateRunMetadata",
  "evaluateReceiptBoundary",
  'run?.status === requirements.required_status',
  'run?.conclusion === requirements.required_conclusion',
  'run?.head_branch === requirements.head_branch',
  'run?.head_sha === receipt?.source_commit',
  'execFileSync("git", ["show", `${commit}:${relativePath}`]',
  'spawnSync("git", ["merge-base", "--is-ancestor", commit, "HEAD"]',
  "receipt source_sha256 does not cover the exact execution required-source set",
  "execution source drift requires new execution",
  "current FBA registry does not match the registry hashed by the execution receipt",
  "static_sanitization_execution_admitted_registry_update_pending",
  "registry_mutated: false",
  "executed_evidence_cleared: false",
  "terminal_inventory_recomputed: false",
  "page_builder_fba_promoted: false",
  "maintainer_external_github_review_required: true",
]) {
  requireText(runner, marker, files.runner);
}
for (const forbidden of [
  "fetch(",
  "https://api.github.com",
  "git push",
  "git commit",
  "updateModuleSettings",
  "compareAndSwapModuleSettings",
  "contents: write",
]) {
  forbidText(runner, forbidden, files.runner);
}

for (const marker of [
  "accepts only the exact completed successful main push run",
  "rejects queued workflow metadata",
  "rejects completed failed workflow metadata",
  "rejects pull request evidence when main push is required",
  "rejects workflow head SHA drift",
  "rejects workflow run identity drift",
  "rejects cryptographic attestation overclaim",
  "accepts the exact execution receipt boundary",
  "rejects test command drift",
  "rejects target pointer drift",
  "rejects receipts that already claim registry mutation",
  "rejects receipts that infer terminal readiness or FBA promotion",
]) {
  requireText(tests, marker, files.tests);
}

for (const required of [files.contract, files.runner, files.tests, files.verifier]) {
  requireValue(
    Array.isArray(contract.required_source_files) && contract.required_source_files.includes(required),
    `${files.contract}: required_source_files is missing ${required}`,
  );
}
requireValue(
  contract.next_cursor?.exact_main_execution ===
      "await_completed_successful_push_run_and_retained_artifact" &&
    contract.next_cursor?.registry_admission ===
      "execute_runner_against_receipt_and_saved_github_run_metadata" &&
    contract.next_cursor?.registry_update ===
      "blocked_on_successful_registry_admission_packet" &&
    contract.next_cursor?.terminal_inventory ===
      "recompute_only_after_evidence_containing_registry_update",
  `${files.contract}: next cursor drifted`,
);

if (failures.length > 0) {
  console.error("[verify-page-builder-static-sanitization-registry-admission] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log("[verify-page-builder-static-sanitization-registry-admission] PASS");
console.log(
  "target=/provider/static_sanitization_contract/executed_evidence; current=pending; exact_main_execution=pending; admission_source=ready; registry_update=blocked",
);
