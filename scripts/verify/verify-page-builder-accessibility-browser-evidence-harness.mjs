#!/usr/bin/env node

import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

function normalizeQuotes(s) {
  return s.replace(/'/g, '"');
}

function requireContains(text, needle, message) {
  if (!text.includes(needle) && !normalizeQuotes(text).includes(normalizeQuotes(needle))) {
    throw new Error(message);
  }
}

function requireAbsent(text, needle, message) {
  if (text.includes(needle)) throw new Error(message);
}

const contractPath =
  "crates/modules/rustok-page-builder/contracts/evidence/page-builder-generic-accessibility-browser-execution-contract.json";
const configPath = "apps/next-admin/playwright.page-builder-accessibility.config.ts";
const setupPath = "apps/next-admin/tests/page-builder-accessibility/global-setup.ts";
const runnerPath = "apps/next-admin/tests/page-builder-accessibility/browser-evidence.spec.ts";

const contractSource = read(contractPath);
const contract = JSON.parse(contractSource);
const config = read(configPath);
const setup = read(setupPath);
const runner = read(runnerPath);
const browserWorkflow = read(".github/workflows/browser-e2e.yml");
const packageJson = JSON.parse(read("apps/next-admin/package.json"));
const pageManager = read("crates/ui/fly/leptos/src/editor.rs");
const capabilityControls = read(
  "crates/modules/rustok-page-builder/admin/src/editor/capability_controls.rs",
);
const renderedEvidence = read(
  "crates/modules/rustok-page-builder/admin/src/ssr_accessibility_evidence_tests.rs",
);

if (contract.status !== "source_ready_maintainer_execution_pending") {
  throw new Error("generic accessibility browser contract must not claim execution");
}
if (
  contract.runner !== runnerPath ||
  contract.config !== configPath ||
  contract.global_setup !== setupPath
) {
  throw new Error("generic accessibility browser contract must point to retained Playwright source");
}
if (
  contract.output?.format !==
  "page_builder_generic_accessibility_browser_execution_v1"
) {
  throw new Error("generic accessibility browser output format drifted");
}
if (
  contract.output?.status !==
  "browser_keyboard_accessibility_tree_passed_screen_reader_pending"
) {
  throw new Error("generic accessibility browser packet must keep screen-reader evidence pending");
}
if (
  contract.deployment_identity?.source_commit_verified_against_checkout_head !== true ||
  contract.deployment_identity?.deployment_digest_is_maintainer_supplied_reviewed_identity !== true ||
  contract.deployment_identity?.browser_independent_digest_to_deployment_attestation !== false ||
  contract.deployment_identity?.deployment_provenance_must_be_verified_outside_this_browser_packet !== true
) {
  throw new Error("generic accessibility browser contract must keep deployment provenance explicit and external");
}
if (JSON.stringify(contract.profiles) !== JSON.stringify(["full", "read_only"])) {
  throw new Error("generic accessibility browser profile matrix drifted");
}
if (contract.fixture_requirements?.minimum_page_count < 2) {
  throw new Error("generic accessibility browser fixture must retain at least two pages");
}
, marker, `accessibility actualization missing ${marker}`);
}

console.log("Page Builder generic accessibility browser evidence harness source: ok");
