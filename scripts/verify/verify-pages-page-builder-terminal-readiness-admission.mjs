#!/usr/bin/env node

import { lstatSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const failures = [];

function read(relativePath) {
  try {
    const absolute = path.join(repoRoot, relativePath);
    const stat = lstatSync(absolute);
    if (!stat.isFile() || stat.isSymbolicLink()) {
      failures.push(`${relativePath}: must be a regular non-symlink file`);
      return "";
    }
    return readFileSync(absolute, "utf8");
  } catch (error) {
    failures.push(`${relativePath}: ${error.message}`);
    return "";
  }
}

function json(relativePath) {
  const source = read(relativePath);
  try {
    return JSON.parse(source);
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

function countPendingExecutedEvidence(value, prefix = "$") {
  const paths = [];
  if (Array.isArray(value)) {
    value.forEach((entry, index) => {
      paths.push(...countPendingExecutedEvidence(entry, `${prefix}[${index}]`));
    });
    return paths;
  }
  if (value === null || typeof value !== "object") return paths;
  for (const [key, nested] of Object.entries(value)) {
    const current = `${prefix}.${key}`;
    if (key === "executed_evidence" && nested === "pending") paths.push(current);
    paths.push(...countPendingExecutedEvidence(nested, current));
  }
  return paths;
}

const contractPath =
  "crates/rustok-page-builder/contracts/evidence/pages-page-builder-terminal-readiness-admission-source.json";
const inventorySourcePath =
  "crates/rustok-page-builder/contracts/evidence/pages-page-builder-terminal-evidence-inventory-source.json";
const runnerPath = "scripts/evidence/admit-pages-page-builder-terminal-readiness.mjs";
const testsPath = "scripts/evidence/admit-pages-page-builder-terminal-readiness.test.mjs";
const verifierPath = "scripts/verify/verify-pages-page-builder-terminal-readiness-admission.mjs";
const actualizationPath =
  "docs/modules/pages-page-builder-terminal-readiness-admission-actualization-2026-08-13.md";
const executionContractPath =
  "crates/rustok-forum/contracts/evidence/forum-page-builder-ffa-fba-promotion-execution-source.json";
const accessibilityContractPath =
  "crates/rustok-page-builder/contracts/evidence/page-builder-generic-accessibility-browser-packet-verifier-source.json";
const registryPath = "docs/modules/registry.md";
const fbaRegistryPath = "crates/rustok-page-builder/contracts/page-builder-fba-registry.json";
const pagesPlanPath = "crates/rustok-pages/docs/implementation-plan.md";

const contract = json(contractPath);
const inventorySource = json(inventorySourcePath);
const executionContract = json(executionContractPath);
const accessibilityContract = json(accessibilityContractPath);
const fbaRegistry = json(fbaRegistryPath);
const runner = read(runnerPath);
const tests = read(testsPath);
const actualization = read(actualizationPath);
const registry = read(registryPath);
const pagesPlan = read(pagesPlanPath);

requireValue(
  contract.format === "pages_page_builder_terminal_readiness_admission_source_v1" &&
    contract.status === "source_ready_maintainer_execution_pending" &&
    contract.scope === "pages_page_builder",
  `${contractPath}: identity drifted`,
);
requireValue(
  contract.potential_terminal_targets?.pages_ffa?.module_slug === "pages" &&
    contract.potential_terminal_targets?.pages_ffa?.readiness_axis === "ffa" &&
    contract.potential_terminal_targets?.pages_ffa?.required_current_registry_status ===
      "in_progress" &&
    contract.potential_terminal_targets?.pages_ffa?.potential_terminal_status ===
      "parity_verified" &&
    contract.potential_terminal_targets?.pages_ffa?.terminal_candidate_ready === false &&
    contract.potential_terminal_targets?.pages_ffa?.structural_shape === "core_transport_ui",
  `${contractPath}: Pages FFA potential target mapping drifted`,
);
requireValue(
  contract.potential_terminal_targets?.page_builder_fba?.module_slug === "page_builder" &&
    contract.potential_terminal_targets?.page_builder_fba?.readiness_axis === "fba" &&
    contract.potential_terminal_targets?.page_builder_fba?.required_current_registry_status ===
      "boundary_ready" &&
    contract.potential_terminal_targets?.page_builder_fba?.potential_terminal_status ===
      "transport_verified" &&
    contract.potential_terminal_targets?.page_builder_fba?.terminal_candidate_ready === false &&
    contract.potential_terminal_targets?.page_builder_fba?.structural_shape === "no_ui_boundary",
  `${contractPath}: Page Builder FBA potential target mapping drifted`,
);
for (const key of [
  "pages_fba_status_mutated",
  "page_builder_ffa_status_mutated",
  "forum_ffa_status_mutated",
  "forum_fba_status_mutated",
  "other_module_status_mutated",
]) {
  requireValue(contract.non_targets?.[key] === false, `${contractPath}: ${key} must remain false`);
}

requireValue(
  contract.promotion_review_input?.format === "forum_page_builder_ffa_fba_promotion_review_v1" &&
    contract.promotion_review_input?.required_status ===
      "owner_approved_ffa_fba_promotion_review_execution_pending" &&
    contract.promotion_review_input?.decision_must_equal === "approve_ffa_fba_promotion_review" &&
    JSON.stringify(contract.promotion_review_input?.targets_must_equal) ===
      JSON.stringify(["ffa", "fba"]),
  `${contractPath}: promotion review input drifted`,
);
requireValue(
  contract.promotion_execution_input?.format === executionContract.output?.format &&
    contract.promotion_execution_input?.required_status === executionContract.output?.success_status &&
    contract.promotion_execution_input?.postcondition_passed_must_be_true === true &&
    contract.promotion_execution_input?.rollback_must_not_have_run === true &&
    contract.promotion_execution_input?.ffa_promoted_must_be_false === true &&
    contract.promotion_execution_input?.fba_promoted_must_be_false === true,
  `${contractPath}: successful promotion execution prerequisite drifted`,
);
requireValue(
  contract.accessibility_input?.format === accessibilityContract.output?.format &&
    contract.accessibility_input?.required_status === accessibilityContract.output?.status &&
    JSON.stringify(contract.accessibility_input?.required_profiles) ===
      JSON.stringify(["full", "read_only"]) &&
    contract.accessibility_input?.owner_review_required_must_be_true === true &&
    contract.accessibility_input?.screen_reader_execution_pending_must_be_true === true &&
    contract.accessibility_input?.wcag_conformance_not_claimed_must_be_true === true,
  `${contractPath}: accessibility prerequisite boundary drifted`,
);

const inventoryGuard = contract.terminal_evidence_inventory_guard ?? {};
requireValue(
  inventoryGuard.page_builder_fba_registry === fbaRegistryPath &&
    inventoryGuard.page_builder_fba_required_current_status === "boundary_ready" &&
    inventoryGuard.page_builder_fba_pending_key === "executed_evidence" &&
    inventoryGuard.page_builder_fba_pending_value === "pending" &&
    inventoryGuard.page_builder_fba_current_pending_entries_must_be_nonzero === true &&
    inventoryGuard.page_builder_fba_pending_entries_block_transport_verified === true &&
    inventoryGuard.pages_ffa_local_plan === pagesPlanPath &&
    inventoryGuard.pages_ffa_current_pending_marker === "execution-rollout-pending" &&
    inventoryGuard.pages_ffa_pending_marker_must_be_present === true &&
    inventoryGuard.pages_ffa_pending_marker_blocks_parity_verified === true &&
    inventoryGuard.complete_terminal_evidence_inventory_required_before_owner_platform_review ===
      true &&
    inventoryGuard.complete_terminal_evidence_inventory_format ===
      "pages_page_builder_terminal_evidence_inventory_v1" &&
    inventoryGuard.complete_terminal_evidence_inventory_source === inventorySourcePath &&
    inventoryGuard.complete_terminal_evidence_inventory_source_format ===
      "pages_page_builder_terminal_evidence_inventory_source_v1" &&
    inventoryGuard.complete_terminal_evidence_inventory_source_defined === true,
  `${contractPath}: terminal evidence inventory guard drifted`,
);
requireValue(
  inventorySource.format === "pages_page_builder_terminal_evidence_inventory_source_v1" &&
    inventorySource.status === "source_ready_maintainer_execution_pending" &&
    inventorySource.output?.format === "pages_page_builder_terminal_evidence_inventory_v1",
  `${inventorySourcePath}: inventory source identity drifted`,
);
const pendingEvidencePaths = countPendingExecutedEvidence(fbaRegistry);
requireValue(
  fbaRegistry.status === "boundary_ready" && pendingEvidencePaths.length > 0,
  `${fbaRegistryPath}: current source no longer has the pending boundary this admission records`,
);
requireText(pagesPlan, "execution-rollout-pending", pagesPlanPath);

requireValue(
  contract.output?.format === "pages_page_builder_terminal_readiness_admission_v1" &&
    contract.output?.status ===
      "rollout_accessibility_prerequisites_admitted_terminal_inventory_pending" &&
    contract.output?.default_path === "target/pages-page-builder-terminal-readiness-admission.json",
  `${contractPath}: output identity drifted`,
);
for (const key of [
  "admission_is_not_terminal_evidence_completion",
  "admission_is_not_owner_approval",
  "admission_is_not_platform_approval",
  "admission_does_not_mutate_registry",
  "admission_does_not_mutate_local_plans",
  "admission_does_not_promote_ffa",
  "admission_does_not_promote_fba",
  "owner_platform_review_remains_blocked_while_terminal_inventory_incomplete",
  "terminal_change_requires_same_pr_local_plan_and_registry_sync",
  "terminal_change_requires_verification_evidence_in_pr",
  "screen_reader_pending_does_not_claim_wcag_conformance",
]) {
  requireValue(contract.governance_boundary?.[key] === true, `${contractPath}: ${key} must remain true`);
}

for (const marker of [
  "validatePromotionReview",
  "validateExecution",
  "validateAccessibility",
  "validateRegistry",
  "validateTerminalEvidenceInventoryGuard",
  "collectPendingEvidence",
  "promotion execution receipt was generated after the retained observed-Wave lease expired",
  "promotion execution does not bind the supplied promotion-review packet",
  "accessibility deployment RepoDigest differs from promotion execution",
  "pending_executed_evidence_count",
  "rollout_accessibility_prerequisites_admitted_terminal_inventory_pending",
  "terminal_evidence_inventory_complete: false",
  "owner_platform_review_ready: false",
  "future_inventory_source_defined: true",
  "future_inventory_source_path",
  "pages_ffa_promoted: false",
  "page_builder_fba_promoted: false",
  "screen_reader_execution_pending: true",
  "wcag_conformance_not_claimed: true",
  "source_mutation_performed: false",
]) {
  requireText(runner, marker, runnerPath);
}
for (const forbidden of [
  "fetch(",
  "@playwright/test",
  "chromium",
  "compareAndSwapModuleSettings",
  "updateModuleSettings",
]) {
  if (runner.includes(forbidden)) failures.push(`${runnerPath}: forbidden mutation/network marker '${forbidden}'`);
}

for (const label of [
  "admits rollout and accessibility prerequisites while retaining incomplete terminal inventory",
  "rejects non-successful promotion execution status",
  "rejects promotion-review decision drift",
  "rejects promotion-review source commit drift",
  "rejects execution source commit drift",
  "rejects accessibility deployment digest drift",
  "rejects a promotion execution that required rollback",
  "rejects failed full accessibility profile",
  "rejects WCAG conformance overclaim",
  "rejects execution generated after observed Wave lease",
  "rejects execution readiness overclaim",
]) {
  requireText(tests, label, testsPath);
}

for (const marker of [
  "rollout-accessibility-prerequisite-admission-source-ready",
  "Pages FFA",
  "Page Builder FBA",
  "parity_verified",
  "transport_verified",
  "terminal_candidate_ready=false",
  "executed_evidence: \"pending\"",
  "execution-rollout-pending",
  "rollout_accessibility_prerequisites_admitted_terminal_inventory_pending",
  "pages_page_builder_terminal_evidence_inventory_v1",
  "owner_platform_review_ready=false",
  "does **not** target Pages FBA, Page Builder FFA, Forum FFA/FBA",
  "No tests, Node verifiers, GraphQL/HTTP calls, live mutations, browser runs, workflows or CI were executed",
]) {
  requireText(actualization, marker, actualizationPath);
}

requireText(registry, contract.registry_precondition.pages_row_required, registryPath);
requireText(registry, contract.registry_precondition.page_builder_row_required, registryPath);
requireText(registry, contract.registry_precondition.source_of_truth_rule_required, registryPath);
requireText(
  registry,
  "If status = `parity_verified` or `transport_verified`, the PR must contain verification evidence.",
  registryPath,
);

for (const required of [
  contractPath,
  runnerPath,
  testsPath,
  verifierPath,
  actualizationPath,
  inventorySourcePath,
]) {
  requireValue(
    Array.isArray(contract.required_source_files) && contract.required_source_files.includes(required),
    `${contractPath}: required_source_files is missing ${required}`,
  );
}
requireValue(
  contract.next_cursor?.rollout_accessibility_prerequisite_admission ===
      "source_ready_blocked_on_successful_execution_and_accessibility_packets" &&
    contract.next_cursor?.terminal_evidence_inventory === "source_ready_maintainer_execution_pending" &&
    contract.next_cursor?.owner_platform_readiness_review ===
      "blocked_on_complete_terminal_evidence_inventory" &&
    contract.next_cursor?.readiness_source_change ===
      "blocked_on_explicit_owner_and_platform_approval_after_complete_inventory",
  `${contractPath}: next cursor drifted`,
);

if (failures.length > 0) {
  console.error("[verify-pages-page-builder-terminal-readiness-admission] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("[verify-pages-page-builder-terminal-readiness-admission] PASS");
console.log(
  `prerequisites=source_ready; page_builder_fba_pending=${pendingEvidencePaths.length}; terminal_inventory_source=defined; owner_platform_review=blocked; source_mutation=false`,
);
