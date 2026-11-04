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
  "crates/modules/rustok-page-builder/contracts/evidence/pages-page-builder-terminal-readiness-admission-source.json";
const inventorySourcePath =
  "crates/modules/rustok-page-builder/contracts/evidence/pages-page-builder-terminal-evidence-inventory-source.json";
const runnerPath = "scripts/evidence/admit-pages-page-builder-terminal-readiness.mjs";
const testsPath = "scripts/evidence/admit-pages-page-builder-terminal-readiness.test.mjs";
const verifierPath = "scripts/verify/verify-pages-page-builder-terminal-readiness-admission.mjs";
const executionContractPath =
  "crates/modules/rustok-forum/contracts/evidence/forum-page-builder-ffa-fba-promotion-execution-source.json";
const accessibilityContractPath =
  "crates/modules/rustok-page-builder/contracts/evidence/page-builder-generic-accessibility-browser-packet-verifier-source.json";
const registryPath = "docs/modules/registry.md";
const fbaRegistryPath = "crates/modules/rustok-page-builder/contracts/page-builder-fba-registry.json";
const pagesPlanPath = "crates/modules/rustok-pages/docs/implementation-plan.md";

const contract = json(contractPath);
const inventorySource = json(inventorySourcePath);
const executionContract = json(executionContractPath);
const accessibilityContract = json(accessibilityContractPath);
const fbaRegistry = json(fbaRegistryPath);
const runner = read(runnerPath);
const tests = read(testsPath);
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
, marker, actualizationPath);
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
