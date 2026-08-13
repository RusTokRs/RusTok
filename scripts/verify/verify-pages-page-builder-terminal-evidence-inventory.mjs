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

function pointerToken(value) {
  return String(value).replaceAll("~", "~0").replaceAll("/", "~1");
}

function collectPendingEvidence(value, prefix = "") {
  const paths = [];
  if (Array.isArray(value)) {
    value.forEach((entry, index) => {
      paths.push(...collectPendingEvidence(entry, `${prefix}/${pointerToken(index)}`));
    });
    return paths;
  }
  if (value === null || typeof value !== "object") return paths;
  for (const [key, nested] of Object.entries(value)) {
    const current = `${prefix}/${pointerToken(key)}`;
    if (key === "executed_evidence" && nested === "pending") paths.push(current);
    paths.push(...collectPendingEvidence(nested, current));
  }
  return paths;
}

const sourcePath =
  "crates/rustok-page-builder/contracts/evidence/pages-page-builder-terminal-evidence-inventory-source.json";
const runnerPath = "scripts/evidence/inventory-pages-page-builder-terminal-readiness.mjs";
const testsPath = "scripts/evidence/inventory-pages-page-builder-terminal-readiness.test.mjs";
const verifierPath = "scripts/verify/verify-pages-page-builder-terminal-evidence-inventory.mjs";
const actualizationPath =
  "docs/modules/pages-page-builder-terminal-evidence-inventory-actualization-2026-08-13.md";
const predecessorSourcePath =
  "crates/rustok-page-builder/contracts/evidence/pages-page-builder-terminal-readiness-admission-source.json";
const predecessorVerifierPath =
  "scripts/verify/verify-pages-page-builder-terminal-readiness-admission.mjs";
const fbaRegistryPath = "crates/rustok-page-builder/contracts/page-builder-fba-registry.json";
const pagesPlanPath = "crates/rustok-pages/docs/implementation-plan.md";
const pageBuilderPlanPath = "crates/rustok-page-builder/docs/implementation-plan.md";
const centralRegistryPath = "docs/modules/registry.md";

const source = json(sourcePath);
const predecessorSource = json(predecessorSourcePath);
const fbaRegistry = json(fbaRegistryPath);
const runner = read(runnerPath);
const tests = read(testsPath);
const actualization = read(actualizationPath);
const predecessorVerifier = read(predecessorVerifierPath);
const pagesPlan = read(pagesPlanPath);
const pageBuilderPlan = read(pageBuilderPlanPath);
const centralRegistry = read(centralRegistryPath);

requireValue(
  source.format === "pages_page_builder_terminal_evidence_inventory_source_v1" &&
    source.status === "source_ready_maintainer_execution_pending" &&
    source.scope === "pages_page_builder",
  `${sourcePath}: identity drifted`,
);
requireValue(
  source.predecessor?.format === "pages_page_builder_terminal_readiness_admission_v1" &&
    source.predecessor?.required_status ===
      "rollout_accessibility_prerequisites_admitted_terminal_inventory_pending" &&
    source.predecessor?.source_commit_must_equal_checkout_head === true &&
    source.predecessor?.retained_admission_source_sha256_must_match_checkout === true &&
    source.predecessor?.retained_inventory_source_sha256_must_match_checkout === true &&
    source.predecessor?.future_inventory_source_binding_must_match === true &&
    source.predecessor?.terminal_evidence_inventory_complete_must_be_false === true &&
    source.predecessor?.owner_platform_review_ready_must_be_false === true &&
    source.predecessor?.source_mutation_performed_must_be_false === true,
  `${sourcePath}: predecessor boundary drifted`,
);
requireValue(
  source.authorities?.central_registry === centralRegistryPath &&
    source.authorities?.pages_local_plan === pagesPlanPath &&
    source.authorities?.page_builder_local_plan === pageBuilderPlanPath &&
    source.authorities?.page_builder_fba_registry === fbaRegistryPath &&
    source.authorities?.prerequisite_admission_source === predecessorSourcePath,
  `${sourcePath}: authority paths drifted`,
);

const currentPendingPaths = collectPendingEvidence(fbaRegistry).sort();
const expectedCurrentPaths = [
  "/provider/consumer_properties_contract/executed_evidence",
  "/provider/static_sanitization_contract/executed_evidence",
  "/consumers/0/metadata_properties/executed_evidence",
  "/consumers/0/artifact_rollback/executed_evidence",
  "/consumers/0/artifact_repair/physical_loss_recovery/rollback_activated_current_set_recovery/executed_evidence",
  "/consumers/0/artifact_repair/physical_loss_recovery/repeated_loss_recovery/executed_evidence",
  "/consumers/0/artifact_repair/physical_loss_recovery/executed_evidence",
  "/consumers/0/artifact_repair/rollback_continuity/physical_loss_activation_prefix/executed_evidence",
  "/consumers/0/artifact_repair/rollback_continuity/rollback_activated_repair_to_rollback/executed_evidence",
  "/consumers/0/artifact_repair/rollback_continuity/executed_evidence",
  "/consumers/0/artifact_repair/executed_evidence",
  "/consumers/0/cache_consumer/executed_evidence",
].sort();
requireValue(
  JSON.stringify(currentPendingPaths) === JSON.stringify(expectedCurrentPaths),
  `${fbaRegistryPath}: current recursive pending evidence inventory changed; actualize the source contract and snapshot`,
);
requireValue(
  fbaRegistry.status === "boundary_ready" &&
    source.page_builder_fba_inventory?.required_registry_status_before_governance === "boundary_ready" &&
    source.page_builder_fba_inventory?.recursive_blocker_key === "executed_evidence" &&
    source.page_builder_fba_inventory?.recursive_blocker_value === "pending" &&
    source.page_builder_fba_inventory?.path_format === "json_pointer" &&
    source.page_builder_fba_inventory?.maximum_blocker_paths === 256 &&
    source.page_builder_fba_inventory?.all_recursive_blocker_paths_must_be_zero_for_completion ===
      true &&
    source.page_builder_fba_inventory?.current_source_expected_to_have_blockers === true &&
    source.page_builder_fba_inventory?.current_source_rechecked_blocker_count === 12 &&
    source.page_builder_fba_inventory?.pending_blockers_prevent_transport_verified === true,
  `${sourcePath}: Page Builder FBA inventory contract drifted`,
);
requireValue(
  pagesPlan.includes("execution-rollout-pending") &&
    source.pages_ffa_inventory?.blocking_status_marker === "execution-rollout-pending" &&
    source.pages_ffa_inventory?.blocking_status_marker_must_be_absent_for_completion === true &&
    source.pages_ffa_inventory?.current_source_expected_to_have_marker === true &&
    source.pages_ffa_inventory?.pending_marker_prevents_parity_verified === true,
  `${sourcePath}: Pages FFA inventory contract drifted`,
);
requireValue(
  typeof pageBuilderPlan === "string" && pageBuilderPlan.length > 0,
  `${pageBuilderPlanPath}: local plan must remain readable`,
);

requireValue(
  source.completion?.requires_valid_predecessor === true &&
    source.completion?.requires_same_exact_source_commit === true &&
    source.completion?.requires_zero_page_builder_fba_recursive_blockers === true &&
    source.completion?.requires_pages_execution_rollout_marker_absent === true &&
    source.completion?.complete_status ===
      "terminal_evidence_inventory_complete_owner_platform_review_ready" &&
    source.completion?.incomplete_status === "terminal_evidence_inventory_incomplete" &&
    source.completion?.complete_means_owner_platform_review_ready_only === true &&
    source.completion?.complete_does_not_mean_owner_approved === true &&
    source.completion?.complete_does_not_mean_platform_approved === true &&
    source.completion?.complete_does_not_promote_ffa === true &&
    source.completion?.complete_does_not_promote_fba === true,
  `${sourcePath}: completion semantics drifted`,
);
requireValue(
  source.output?.format === "pages_page_builder_terminal_evidence_inventory_v1" &&
    source.output?.default_path === "target/pages-page-builder-terminal-evidence-inventory.json" &&
    source.output?.page_builder_fba_blocker_paths_retained === true &&
    source.output?.pages_execution_rollout_marker_presence_retained === true &&
    source.output?.owner_platform_review_ready_retained === true,
  `${sourcePath}: output contract drifted`,
);

for (const key of [
  "inventory_is_not_owner_approval",
  "inventory_is_not_platform_approval",
  "inventory_does_not_mutate_registry",
  "inventory_does_not_mutate_local_plans",
  "inventory_does_not_promote_ffa",
  "inventory_does_not_promote_fba",
  "owner_platform_review_must_remain_blocked_when_incomplete",
  "terminal_change_requires_same_pr_local_plan_and_registry_sync",
  "terminal_change_requires_verification_evidence_in_pr",
]) {
  requireValue(source.governance_boundary?.[key] === true, `${sourcePath}: ${key} must remain true`);
}

requireValue(
  predecessorSource.terminal_evidence_inventory_guard
    ?.complete_terminal_evidence_inventory_source_defined === true &&
    predecessorSource.terminal_evidence_inventory_guard
      ?.complete_terminal_evidence_inventory_source === sourcePath &&
    predecessorSource.terminal_evidence_inventory_guard
      ?.complete_terminal_evidence_inventory_source_format ===
      "pages_page_builder_terminal_evidence_inventory_source_v1" &&
    predecessorSource.next_cursor?.terminal_evidence_inventory ===
      "source_ready_maintainer_execution_pending" &&
    Array.isArray(predecessorSource.required_source_files) &&
    predecessorSource.required_source_files.includes(sourcePath),
  `${predecessorSourcePath}: predecessor cursor is not actualized to the inventory source`,
);
for (const marker of [
  "complete_terminal_evidence_inventory_source_defined === true",
  "terminal evidence inventory source identity does not match the admission guard",
  "future_inventory_source_defined: true",
  "future_inventory_source_path",
]) {
  requireText(predecessorVerifier, marker, predecessorVerifierPath);
}

for (const marker of [
  "collectPendingEvidence",
  "evaluateInventory",
  "--prerequisite-admission",
  "prerequisite admission source_commit does not equal checkout HEAD",
  "prerequisite retained admission-source hash does not match checkout",
  "prerequisite retained inventory-source hash does not match checkout",
  "prerequisite terminal-inventory source binding does not match checkout",
  "prerequisite admission contains a terminal-readiness overclaim",
  "prerequisite Page Builder FBA blocker count does not match same-source canonical registry",
  "prerequisite Pages rollout blocker fact does not match same-source canonical plan",
  "pending_executed_evidence_paths",
  "terminal_evidence_inventory_complete_owner_platform_review_ready",
  "terminal_evidence_inventory_incomplete",
  "owner_platform_review_ready",
  "pages_ffa_promoted: false",
  "page_builder_fba_promoted: false",
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
  "git push",
]) {
  if (runner.includes(forbidden)) failures.push(`${runnerPath}: forbidden network/mutation marker '${forbidden}'`);
}

for (const marker of [
  "collects every nested executed-evidence blocker as a stable JSON Pointer",
  "escapes JSON Pointer tokens",
  "ignores unrelated pending values",
  "remains incomplete while Page Builder FBA blockers exist",
  "remains incomplete while Pages execution rollout marker exists",
  "requires a valid predecessor even when canonical blockers are clear",
  "becomes review-ready only when predecessor and both canonical blocker sets are clear",
  "does not infer readiness from a reduced but nonzero blocker set",
]) {
  requireText(tests, marker, testsPath);
}

for (const marker of [
  "terminal-evidence-inventory-source-ready",
  "12",
  "/provider/consumer_properties_contract/executed_evidence",
  "/consumers/0/cache_consumer/executed_evidence",
  "execution-rollout-pending",
  "terminal_evidence_inventory_incomplete",
  "terminal_evidence_inventory_complete_owner_platform_review_ready",
  "review-ready only",
  "retained source hashes",
  "owner_platform_review_ready=false",
  "No tests, Node verifiers, Cargo commands, GraphQL/HTTP calls, live mutations, browser runs, workflows or CI were executed",
]) {
  requireText(actualization, marker, actualizationPath);
}

for (const marker of [
  source.readiness_precondition.pages_row_required,
  source.readiness_precondition.page_builder_row_required,
  source.readiness_precondition.source_of_truth_rule_required,
  "If status = `parity_verified` or `transport_verified`, the PR must contain verification evidence.",
]) {
  requireText(centralRegistry, marker, centralRegistryPath);
}

for (const required of [sourcePath, runnerPath, testsPath, verifierPath, actualizationPath]) {
  requireValue(
    Array.isArray(source.required_source_files) && source.required_source_files.includes(required),
    `${sourcePath}: required_source_files is missing ${required}`,
  );
}
requireValue(
  source.next_cursor?.terminal_evidence_inventory_source ===
      "source_ready_maintainer_execution_pending" &&
    source.next_cursor?.remaining_execution_evidence ===
      "execute_and_retain_each_canonical_blocker_before_source_status_clearance" &&
    source.next_cursor?.owner_platform_readiness_review ===
      "blocked_on_terminal_evidence_inventory_complete_owner_platform_review_ready" &&
    source.next_cursor?.readiness_source_change ===
      "blocked_on_explicit_owner_and_platform_approval_after_complete_inventory",
  `${sourcePath}: next cursor drifted`,
);

if (failures.length > 0) {
  console.error("[verify-pages-page-builder-terminal-evidence-inventory] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("[verify-pages-page-builder-terminal-evidence-inventory] PASS");
console.log(
  `current_fba_blockers=${currentPendingPaths.length}; pages_rollout_pending=true; terminal_inventory_source=ready; owner_platform_review=blocked`,
);
