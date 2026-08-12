#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const paths = {
  verifierContract:
    "crates/rustok-page-builder/contracts/evidence/page-builder-generic-accessibility-browser-packet-verifier-source.json",
  executionContract:
    "crates/rustok-page-builder/contracts/evidence/page-builder-generic-accessibility-browser-execution-contract.json",
  runner: "scripts/evidence/verify-page-builder-accessibility-browser-packet.mjs",
  runnerTest: "scripts/evidence/verify-page-builder-accessibility-browser-packet.test.mjs",
  actualization: "docs/modules/pages-page-builder-parity-accessibility-actualization-2026-08-12.md",
  workflow: ".github/workflows/pages-page-builder-parity.yml",
};
const failures = [];

function abs(relativePath) {
  return path.join(repoRoot, relativePath);
}

function read(relativePath) {
  const absolutePath = abs(relativePath);
  if (!fs.existsSync(absolutePath)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  const stat = fs.lstatSync(absolutePath);
  if (!stat.isFile() || stat.isSymbolicLink()) {
    failures.push(`${relativePath}: must be a regular non-symlink file`);
    return "";
  }
  return fs.readFileSync(absolutePath, "utf8");
}

function need(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing '${marker}'`);
}

function forbid(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden '${marker}'`);
}

const verifierContractSource = read(paths.verifierContract);
const executionContractSource = read(paths.executionContract);
const runner = read(paths.runner);
const runnerTest = read(paths.runnerTest);
const actualization = read(paths.actualization);
const workflow = read(paths.workflow);

let verifierContract = {};
let executionContract = {};
try {
  verifierContract = JSON.parse(verifierContractSource);
} catch (error) {
  failures.push(`${paths.verifierContract}: invalid JSON: ${error}`);
}
try {
  executionContract = JSON.parse(executionContractSource);
} catch (error) {
  failures.push(`${paths.executionContract}: invalid JSON: ${error}`);
}

if (
  verifierContract.schema_version !== 1 ||
  verifierContract.module !== "page-builder" ||
  verifierContract.packet !== "generic_editor_accessibility_browser_packet_verifier" ||
  verifierContract.format !== "page_builder_generic_accessibility_browser_packet_verifier_source_v1" ||
  verifierContract.status !== "source_ready_maintainer_execution_pending"
) {
  failures.push("accessibility browser packet verifier source identity drifted");
}

for (const [key, expected] of [
  ["execution_contract", paths.executionContract],
  ["required_format", "page_builder_generic_accessibility_browser_execution_v1"],
  ["required_status", "browser_keyboard_accessibility_tree_passed_screen_reader_pending"],
  ["source_commit_must_equal_checkout_head", true],
  ["expected_deployment_digest_must_be_supplied_separately", true],
  ["packet_deployment_digest_must_equal_expected", true],
  ["retained_source_hashes_must_match_contract_and_checkout", true],
  ["all_required_profiles_must_pass", true],
  ["critical_failures_must_be_zero", true],
  ["privacy_non_claim_flags_must_remain_fail_closed", true],
]) {
  if (verifierContract.input?.[key] !== expected) failures.push(`verifier input ${key} drifted`);
}

if (
  executionContract.output?.format !== verifierContract.input?.required_format ||
  executionContract.output?.status !== verifierContract.input?.required_status
) {
  failures.push("browser execution contract and packet verifier input identity drifted");
}

const expectedFacts = {
  full: [
    "tabFocusBetweenAdjacentPages",
    "keyboardActivationUpdatedPressedState",
    "addPageSequentialFocusOrder",
    "ariaTreePressedStateObserved",
    "ariaTreeAddPageNameObserved",
    "pageMetadataAccessibleNamesResolved",
  ],
  read_only: [
    "editFieldsetBrowserDisabled",
    "editFieldsetAriaDisabled",
    "propertiesFieldsetBrowserDisabled",
    "propertiesFieldsetAriaDisabled",
    "mutationControlsBrowserDisabled",
    "pageNavigationKeyboardAvailable",
  ],
};
for (const profile of ["full", "read_only"]) {
  if (verifierContract.profiles?.[profile]?.minimum_page_count !== 2) {
    failures.push(`${profile} verifier minimum page count drifted`);
  }
  if (
    JSON.stringify(verifierContract.profiles?.[profile]?.required_boolean_facts) !==
    JSON.stringify(expectedFacts[profile])
  ) {
    failures.push(`${profile} verifier required facts drifted`);
  }
}

for (const [flag, expected] of [
  ["retained_secrets", false],
  ["raw_dom_retained", false],
  ["aria_snapshot_text_retained", false],
  ["screen_reader_execution_pending", true],
  ["wcag_conformance_not_claimed", true],
]) {
  if (verifierContract.required_input_flags?.[flag] !== expected) {
    failures.push(`required input flag ${flag} drifted`);
  }
}

if (
  verifierContract.output?.format !==
    "page_builder_generic_accessibility_browser_packet_verification_v1" ||
  verifierContract.output?.status !==
    "browser_packet_verified_owner_review_ready_screen_reader_pending" ||
  verifierContract.output?.default_path !==
    "target/page-builder-accessibility-browser-packet-verification.json"
) {
  failures.push("packet verifier output identity drifted");
}

for (const [key, expected] of [
  ["expected_digest_is_maintainer_supplied_reviewed_identity", true],
  ["digest_equality_is_not_cryptographic_origin_binding", true],
  ["deployment_provenance_must_be_verified_outside_this_verifier", true],
]) {
  if (verifierContract.deployment_identity?.[key] !== expected) {
    failures.push(`deployment identity boundary ${key} drifted`);
  }
}

for (const nonClaim of [
  "browser execution by the packet verifier",
  "screen-reader execution",
  "WCAG conformance",
  "cryptographic origin-to-RepoDigest binding",
  "provider SLO health",
  "Pages gate acceptance",
  "Forum Wave admission",
  "tenant rollout or promotion",
]) {
  if (!(verifierContract.not_claimed ?? []).includes(nonClaim)) {
    failures.push(`packet verifier non-claim missing ${nonClaim}`);
  }
}

if (
  verifierContract.runner !== paths.runner ||
  verifierContract.runner_test !== paths.runnerTest ||
  verifierContract.source_verifier !==
    "scripts/verify/verify-page-builder-accessibility-browser-packet-verifier.mjs"
) {
  failures.push("packet verifier source path binding drifted");
}

for (const marker of [
  'execFileSync("git", ["rev-parse", "HEAD"]',
  '"--expected-source"',
  '"--expected-deployment-digest"',
  "requireExactKeys(packet.source_files, required, \"packet source_files\")",
  "retained source hash does not match checkout",
  "browser packet deployment_digest does not match the separately supplied expected RepoDigest",
  "verifyObservation(profile, packet.observations[profile])",
  "screen_reader_execution_pending: true",
  "wcag_conformance_not_claimed: true",
  "deployment_provenance_verified_by_this_packet: false",
  "cryptographic_origin_to_repo_digest_binding_claimed: false",
  "owner_review_required: true",
  "provider_slo_health_not_claimed: true",
  "pages_gate_acceptance_not_claimed: true",
  "forum_wave_admission_not_claimed: true",
  "tenant_rollout_not_claimed: true",
]) {
  need(runner, marker, "packet verifier runner");
}
for (const marker of [
  "fetch(",
  "@playwright/test",
  'spawnSync("cargo"',
  "playwright test",
  "screen_reader_execution_pending: false",
  "wcag_conformance_not_claimed: false",
  "deployment_provenance_verified_by_this_packet: true",
  "cryptographic_origin_to_repo_digest_binding_claimed: true",
  "owner_review_required: false",
]) {
  forbid(runner, marker, "packet verifier runner");
}

for (const marker of [
  'requireSuccess("valid"',
  'requireFailure("source-tamper"',
  'requireFailure("screen-reader-overclaim"',
  'requireFailure("wcag-overclaim"',
  'requireFailure("missing-fact"',
  'requireFailure("retained-data-drift"',
  '"digest-mismatch"',
  "PASS cases=7",
]) {
  need(runnerTest, marker, "packet verifier synthetic tests");
}

for (const marker of [
  "generic-accessibility-browser-packet-verifier-source-ready",
  paths.verifierContract,
  paths.runner,
  paths.runnerTest,
  "browser_packet_verified_owner_review_ready_screen_reader_pending",
  "owner_review_required = true",
  "deployment_provenance_verified_by_this_packet = false",
  "screen_reader_execution_pending = true",
  "wcag_conformance_not_claimed = true",
  "synthetic test creates no deployment claim",
]) {
  need(actualization, marker, "accessibility actualization");
}

for (const marker of [
  paths.verifierContract,
  paths.runner,
  paths.runnerTest,
  "scripts/verify/verify-page-builder-accessibility-browser-packet-verifier.mjs",
  "Verify generic editor accessibility browser packet verifier",
  "Exercise generic editor accessibility browser packet verifier",
]) {
  need(workflow, marker, "focused parity workflow");
}

if (failures.length > 0) {
  console.error("[verify-page-builder-accessibility-browser-packet-verifier] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "[verify-page-builder-accessibility-browser-packet-verifier] PASS source_ready=true execution=pending owner_review=pending screen_reader=pending",
);
