#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const failures = [];
const files = {
  contract:
    "crates/rustok-pages/contracts/evidence/pages-consumer-properties-provider-admission-source.json",
  runner: "scripts/evidence/admit-pages-consumer-properties-provider.mjs",
  test: "scripts/evidence/admit-pages-consumer-properties-provider.test.mjs",
  rust: "crates/rustok-pages/contracts/evidence/pages-consumer-properties-source-execution.json",
  browser:
    "crates/rustok-pages/contracts/evidence/pages-published-metadata-browser-execution-contract.json",
  deployment:
    "crates/rustok-page-builder/contracts/evidence/page-builder-provider-health-deployment-identity-source.json",
  consumer: "crates/rustok-page-builder/contracts/page-builder-consumer-properties.json",
  registry: "crates/rustok-page-builder/contracts/page-builder-fba-registry.json",
  actualization:
    "docs/modules/pages-consumer-properties-provider-admission-actualization-2026-08-18.md",
  workflow: ".github/workflows/fly-page-builder.yml",
};
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
  console.error("[verify-pages-consumer-properties-provider-admission] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const contract = JSON.parse(read(files.contract));
const rust = JSON.parse(read(files.rust));
const browser = JSON.parse(read(files.browser));
const deployment = JSON.parse(read(files.deployment));
const consumer = JSON.parse(read(files.consumer));
const registry = JSON.parse(read(files.registry));
const runner = read(files.runner);
const test = read(files.test);
const actualization = read(files.actualization);

if (
  contract.format !== "pages_consumer_properties_provider_admission_source_v1" ||
  contract.status !== "source_ready_maintainer_execution_pending"
) {
  failures.push("provider admission source identity drifted");
}

for (const [object, key, expected] of [
  [contract.rust_receipt_input, "format", rust.output?.format],
  [contract.rust_receipt_input, "required_status", rust.output?.success_status],
  [contract.rust_receipt_input, "source_commit_must_be_ancestor_of_checkout", true],
  [contract.rust_receipt_input, "retained_source_hashes_must_match_checkout", true],
  [contract.browser_input, "format", browser.output?.format],
  [contract.browser_input, "required_status", browser.output?.status],
  [contract.browser_input, "critical_failures_must_be_zero", true],
  [contract.deployment_identity_input, "format", "page_builder_provider_health_deployment_identity_v1"],
  [
    contract.deployment_identity_input,
    "required_status",
    "deployment_identity_verified_health_evaluation_pending",
  ],
  [contract.deployment_identity_input, "source_commit_must_equal_browser_source_commit", true],
  [contract.deployment_identity_input, "deployment_digest_must_equal_browser_digest", true],
  [contract.lineage, "rust_source_commit_must_be_ancestor_of_browser_source_commit", true],
  [contract.lineage, "all_three_packet_source_sets_must_match_current_checkout", true],
  [contract.lineage, "required_source_drift_fails_closed", true],
  [contract.output, "format", "pages_consumer_properties_provider_admission_v1"],
  [
    contract.output,
    "status",
    "provider_consumer_properties_inputs_admitted_registry_update_pending",
  ],
  [contract.output, "raw_input_paths_retained", false],
  [contract.governance_boundary, "admission_packet_mutates_consumer_contract", false],
  [contract.governance_boundary, "admission_packet_mutates_fba_registry", false],
  [contract.governance_boundary, "separate_evidence_containing_update_required", true],
  [contract.governance_boundary, "terminal_inventory_complete_claimed", false],
  [contract.governance_boundary, "pages_ffa_promoted", false],
  [contract.governance_boundary, "page_builder_fba_promoted", false],
]) {
  if (object?.[key] !== expected) failures.push(`${key} drifted`);
}

if (
  contract.rust_receipt_input.source_contract !== files.rust ||
  contract.browser_input.source_contract !== files.browser ||
  contract.deployment_identity_input.source_contract !== files.deployment
) {
  failures.push("predecessor contract path drifted");
}
if (
  contract.targets?.consumer_contract?.path !== files.consumer ||
  contract.targets?.consumer_contract?.json_pointer !== "/executed_evidence" ||
  contract.targets?.consumer_contract?.required_before !== "pending"
) {
  failures.push("consumer contract admission target drifted");
}
if (
  contract.targets?.fba_registry?.path !== files.registry ||
  contract.targets?.fba_registry?.json_pointer !==
    "/provider/consumer_properties_contract/executed_evidence" ||
  contract.targets?.fba_registry?.required_before !== "pending"
) {
  failures.push("FBA registry admission target drifted");
}
if (consumer.executed_evidence !== "pending") {
  failures.push("consumer contract must remain pending in source-only admission slice");
}
if (registry.provider?.consumer_properties_contract?.executed_evidence !== "pending") {
  failures.push("FBA provider consumer-properties must remain pending in source-only admission slice");
}

for (const requiredSource of [
  files.contract,
  files.runner,
  files.test,
  files.actualization,
  files.workflow,
  files.rust,
  files.browser,
  files.deployment,
  files.consumer,
  files.registry,
]) {
  if (!(contract.required_source_files ?? []).includes(requiredSource)) {
    failures.push(`required_source_files missing ${requiredSource}`);
  }
}

for (const marker of [
  'spawnSync("git", ["merge-base", "--is-ancestor"',
  'verifyRetainedSourceHashes(document, rustContract, "source_sha256", "Rust receipt")',
  'verifyRetainedSourceHashes(document, browserContract, "source_files", "browser packet")',
  'verifyRetainedSourceHashes(document, deploymentContract, "source_files", "deployment identity")',
  "deployment image digest differs from browser packet",
  "rustSourceCommit",
  "browserLineage.sourceCommit",
  "validateCurrentTargets(admissionContract)",
  "status: admissionContract.output.status",
  "consumer_contract_mutated: false",
  "fba_registry_mutated: false",
  "separate_evidence_containing_update_required: true",
]) {
  need(runner, marker, "provider admission runner");
}
for (const marker of [
  "fetch(",
  "@playwright/test",
  'spawnSync("cargo"',
  "executed_evidence =",
  'executed_evidence: "verified"',
  "pages_ffa_promoted: true",
  "page_builder_fba_promoted: true",
]) {
  forbid(runner, marker, "provider admission runner");
}

for (const marker of [
  "rust-source-hash",
  "rust-pr-provenance",
  "browser-digest",
  "browser-profile",
  "deployment-incomplete",
  "deployment-target-source",
  "deployment-privacy",
  "promotion-boundary",
  "positive=1 fail_closed=8",
]) {
  need(test, marker, "provider admission runner test");
}


const workflow = read(files.workflow);
for (const marker of [
  "scripts/evidence/admit-pages-consumer-properties-provider*.mjs",
  "scripts/verify/verify-pages-consumer-properties-provider-admission.mjs",
  "Verify consumer-properties provider admission source",
  "Test consumer-properties provider admission fail-closed runner",
  "node scripts/verify/verify-pages-consumer-properties-provider-admission.mjs",
  "node --test scripts/evidence/admit-pages-consumer-properties-provider.test.mjs",
]) {
  need(workflow, marker, "Fly Page Builder workflow");
}

for (const marker of [
  "source-ready / live-input-execution-pending / registry-update-blocked",
  "Rust receipt",
  "published-metadata browser packet",
  "deployment identity",
  "maintainer_reviewed_external_fact",
  "does not change `executed_evidence`",
  "terminal inventory remains `1`",
  "execution-rollout-pending",
]) {
  need(actualization, marker, "provider admission actualization");
}

if (contract.next_cursor?.admission_runner !== "source_ready_live_inputs_pending") {
  failures.push("provider admission next cursor drifted");
}
if (contract.next_cursor?.terminal_inventory_recompute !== "blocked_on_separate_registry_update") {
  failures.push("terminal inventory next cursor drifted");
}

if (failures.length) {
  console.error("[verify-pages-consumer-properties-provider-admission] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "[verify-pages-consumer-properties-provider-admission] PASS source_ready=true live_inputs=pending registry_update=blocked",
);
