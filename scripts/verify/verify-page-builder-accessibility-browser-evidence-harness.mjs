#!/usr/bin/env node

import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

function requireContains(text, needle, message) {
  if (!text.includes(needle)) throw new Error(message);
}

function requireAbsent(text, needle, message) {
  if (text.includes(needle)) throw new Error(message);
}

const contractPath =
  "crates/rustok-page-builder/contracts/evidence/page-builder-generic-accessibility-browser-execution-contract.json";
const configPath = "apps/next-admin/playwright.page-builder-accessibility.config.ts";
const setupPath = "apps/next-admin/tests/page-builder-accessibility/global-setup.ts";
const runnerPath = "apps/next-admin/tests/page-builder-accessibility/browser-evidence.spec.ts";
const packetPath =
  "docs/modules/pages-page-builder-parity-accessibility-actualization-2026-08-12.md";

const contractSource = read(contractPath);
const contract = JSON.parse(contractSource);
const config = read(configPath);
const setup = read(setupPath);
const runner = read(runnerPath);
const packet = read(packetPath);
const packageJson = JSON.parse(read("apps/next-admin/package.json"));
const pageManager = read("crates/rustok-page-builder/admin/src/editor/page_manager.rs");
const capabilityControls = read(
  "crates/rustok-page-builder/admin/src/editor/capability_controls.rs",
);
const renderedEvidence = read(
  "crates/rustok-page-builder/admin/src/ssr_accessibility_evidence_tests.rs",
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
for (const pending of [
  "browser execution before a retained passing packet exists",
  "screen-reader execution",
  "WCAG conformance",
  "browser-independent digest-to-deployment attestation",
  "provider SLO health",
  "Pages gate acceptance",
  "Forum Wave admission",
]) {
  if (!contract.not_claimed?.includes(pending)) {
    throw new Error(`generic accessibility browser contract must keep ${pending} unclaimed`);
  }
}

for (const marker of [
  "RUSTOK_PAGE_BUILDER_ACCESSIBILITY_SOURCE_COMMIT",
  "RUSTOK_PAGE_BUILDER_ACCESSIBILITY_DEPLOYMENT_DIGEST",
  "RUSTOK_PAGE_BUILDER_ACCESSIBILITY_EDITOR_STORAGE_STATE",
  "RUSTOK_PAGE_BUILDER_ACCESSIBILITY_FULL_URL",
  "RUSTOK_PAGE_BUILDER_ACCESSIBILITY_READ_ONLY_URL",
]) {
  requireContains(contractSource, marker, `accessibility browser contract missing ${marker}`);
}

for (const marker of [
  'globalSetup: "./tests/page-builder-accessibility/global-setup.ts"',
  "fullyParallel: false",
  "retries: 0",
  "workers: 1",
  'trace: "off"',
  'screenshot: "off"',
  'video: "off"',
  'name: "page-builder-accessibility-chromium"',
]) {
  requireContains(config, marker, `accessibility Playwright config missing ${marker}`);
}

for (const marker of [
  "contract.output.environment",
  "contract.output.default_path",
  'path.resolve(repoRoot, "target")',
  "rmSync(resolveOutput(contract), { force: true })",
]) {
  requireContains(setup, marker, `accessibility browser setup missing ${marker}`);
}
for (const forbidden of ["readFileSync(resolveOutput", "writeFileSync", "renameSync"]) {
  requireAbsent(setup, forbidden, `global setup must only clear output: ${forbidden}`);
}

for (const marker of [
  'button[aria-pressed]',
  'page.keyboard.press(forward ? "Tab" : "Shift+Tab")',
  'page.keyboard.press("Enter")',
  'toMatchAriaSnapshot(`- button [pressed=true]`)',
  'textbox "Add page: Page name"',
  'name: "Page name"',
  'name: "Page id"',
  'fieldset[data-fly-capability=\'edit\']',
  'fieldset[data-fly-capability=\'properties\']',
  'toHaveAttribute("aria-disabled", "true")',
  "toBeDisabled()",
  "source_files: inputs.sourceHashes",
  "profile_url_sha256: inputs.routeHashes",
  "retained_secrets: false",
  "raw_dom_retained: false",
  "aria_snapshot_text_retained: false",
  "screen_reader_execution_pending: true",
  "wcag_conformance_not_claimed: true",
]) {
  requireContains(runner, marker, `accessibility browser runner missing ${marker}`);
}
for (const forbidden of [
  ".content()",
  ".cookies()",
  "response.text()",
  "response.body()",
  "storageState({ path:",
  "screenshot(",
  "tracing.start",
  ".ariaSnapshot()",
]) {
  requireAbsent(
    runner,
    forbidden,
    `accessibility browser runner must not retain sensitive browser material through ${forbidden}`,
  );
}

if (packageJson.devDependencies?.["@playwright/test"] === undefined) {
  throw new Error("generic accessibility browser harness must reuse the existing Playwright dependency");
}

for (const marker of [
  "aria-pressed=active.to_string()",
  "aria-label=new_page_name_accessible_label",
  "<span class=\"font-medium\">{name_label.clone()}</span>",
  "<span class=\"font-medium\">{id_label}</span>",
]) {
  requireContains(pageManager, marker, `page manager browser boundary missing ${marker}`);
}
for (const marker of [
  "disabled=move || !disabled_runtime.capability_enabled(capability)",
  "aria-disabled=move || (!enabled_runtime.capability_enabled(capability)).to_string()",
  "data-fly-capability=capability_id",
]) {
  requireContains(
    capabilityControls,
    marker,
    `capability browser boundary missing ${marker}`,
  );
}
for (const marker of [
  "generic_editor_ssr_exposes_selected_page_and_programmatic_page_name",
  "generic_editor_ssr_exposes_denied_capabilities_as_disabled_fieldsets",
]) {
  requireContains(renderedEvidence, marker, `rendered evidence continuity missing ${marker}`);
}
for (const marker of [
  "generic-accessibility-browser-harness-source-ready",
  "page_builder_generic_accessibility_browser_execution_v1",
  "maintainer browser execution pending",
  "screen-reader execution remains pending",
  "WCAG conformance remains unclaimed",
]) {
  requireContains(packet, marker, `accessibility actualization missing ${marker}`);
}

console.log("Page Builder generic accessibility browser evidence harness source: ok");
