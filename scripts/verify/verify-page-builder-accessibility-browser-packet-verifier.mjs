#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const paths = {
  verifierContract:
    "crates/modules/rustok-page-builder/contracts/evidence/page-builder-generic-accessibility-browser-packet-verifier-source.json",
  executionContract:
    "crates/modules/rustok-page-builder/contracts/evidence/page-builder-generic-accessibility-browser-execution-contract.json",
  runner: "scripts/evidence/verify-page-builder-accessibility-browser-packet.mjs",
  runnerTest: "scripts/evidence/verify-page-builder-accessibility-browser-packet.test.mjs",
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
, marker, "accessibility actualization");
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
