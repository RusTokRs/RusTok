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
  "crates/rustok-forum/contracts/evidence/forum-page-builder-browser-execution-contract.json";
const configPath = "apps/next-admin/playwright.forum-page-builder.config.ts";
const setupPath = "apps/next-admin/tests/forum-page-builder/global-setup.ts";
const runnerPath = "apps/next-admin/tests/forum-page-builder/browser-evidence.spec.ts";
const packetPath =
  "docs/modules/forum-page-builder-browser-evidence-harness-actualization-2026-08-08.md";
const contractSource = read(contractPath);
const contract = JSON.parse(contractSource);
const config = read(configPath);
const setup = read(setupPath);
const runner = read(runnerPath);
const packet = read(packetPath);
const packageJson = JSON.parse(read("apps/next-admin/package.json"));
const moduleManifest = read("crates/rustok-forum/rustok-module.toml");
const propertyPanel = read(
  "crates/rustok-page-builder/admin/src/editor/contribution_properties.rs",
);
const previewPanel = read(
  "crates/rustok-page-builder/admin/src/editor/contribution_preview.rs",
);
const palette = read("crates/rustok-page-builder/admin/src/editor/palette_layers.rs");
const composition = read("apps/admin/src/app/page_builder_contributions.rs");

if (contract.status !== "source_ready_maintainer_execution_pending") {
  throw new Error("browser evidence contract must not claim execution");
}
if (
  contract.runner !== runnerPath ||
  contract.config !== configPath ||
  contract.global_setup !== setupPath
) {
  throw new Error("browser evidence contract must point to the retained Playwright source");
}
if (contract.output?.format !== "forum_page_builder_browser_execution_v1") {
  throw new Error("browser evidence contract output format drifted");
}
if (
  contract.output?.status !==
  "browser_execution_passed_runtime_evidence_pending"
) {
  throw new Error("browser packet must keep runtime evidence pending");
}
if (
  contract.deployment_identity?.source_commit_verified_against_checkout_head !== true ||
  contract.deployment_identity?.deployment_digest_is_maintainer_supplied_reviewed_identity !== true ||
  contract.deployment_identity?.browser_independent_digest_to_deployment_attestation !== false ||
  contract.deployment_identity?.deployment_provenance_must_be_verified_outside_this_browser_packet !== true
) {
  throw new Error("browser evidence contract must keep deployment provenance explicit and external");
}
const expectedProfiles = [
  "full",
  "preview_off",
  "properties_off",
  "forum_disabled",
  "no_read",
];
if (JSON.stringify(contract.profiles) !== JSON.stringify(expectedProfiles)) {
  throw new Error("browser evidence profile matrix drifted");
}
for (const pending of [
  "browser execution",
  "browser-independent digest-to-deployment attestation",
  "runtime authorization execution",
  "observed Page Builder Wave",
  "provider SLO health",
]) {
  if (!contract.not_claimed?.includes(pending)) {
    throw new Error(`browser evidence contract must keep ${pending} pending`);
  }
}

for (const marker of [
  "RUSTOK_FORUM_PAGE_BUILDER_BROWSER_SOURCE_COMMIT",
  "RUSTOK_FORUM_PAGE_BUILDER_DEPLOYMENT_DIGEST",
  "RUSTOK_FORUM_PAGE_BUILDER_EDITOR_STORAGE_STATE",
  "RUSTOK_FORUM_PAGE_BUILDER_NO_READ_STORAGE_STATE",
  "RUSTOK_FORUM_PAGE_BUILDER_FULL_URL",
  "RUSTOK_FORUM_PAGE_BUILDER_PREVIEW_OFF_URL",
  "RUSTOK_FORUM_PAGE_BUILDER_PROPERTIES_OFF_URL",
  "RUSTOK_FORUM_PAGE_BUILDER_FORUM_DISABLED_URL",
  "RUSTOK_FORUM_PAGE_BUILDER_NO_READ_URL",
]) {
  requireContains(contractSource, marker, `browser contract missing environment: ${marker}`);
}

for (const marker of [
  'globalSetup: "./tests/forum-page-builder/global-setup.ts"',
  "fullyParallel: false",
  "retries: 0",
  "workers: 1",
  'trace: "off"',
  'screenshot: "off"',
  'video: "off"',
  'name: "forum-page-builder-chromium"',
]) {
  requireContains(config, marker, `Playwright config missing ${marker}`);
}

for (const marker of [
  "contract.output.environment",
  "contract.output.default_path",
  'path.resolve(repoRoot, "target")',
  "rmSync(resolveOutput(contract), { force: true })",
]) {
  requireContains(setup, marker, `browser global setup missing stale-output guard: ${marker}`);
}
for (const forbidden of ["readFileSync(resolveOutput", "writeFileSync", "renameSync"])
  requireAbsent(setup, forbidden, `global setup must only clear the evidence output: ${forbidden}`);

for (const marker of [
  "forum.topic_list",
  "forum.topic_list.v1",
  "fly-owner-property-per_page",
  "fly-owner-property-category_id",
  'fill("101")',
  "Owner validation rejected the current widget properties",
  "Owner-normalized properties applied to the Fly draft",
  "intent:undo",
  "intent:redo",
  "data-page-builder-contribution-preview-result='ready'",
  "intent:save",
  "preview_off",
  "properties_off",
  "forum_disabled",
  "no_read",
  "source_files: inputs.sourceHashes",
  "profile_url_sha256: inputs.routeHashes",
  "retained_secrets: false",
  "runtime_authorization_evidence_pending: true",
  "observed_page_builder_wave_pending: true",
]) {
  requireContains(runner, marker, `browser harness missing required marker: ${marker}`);
}

for (const forbidden of [
  ".content()",
  ".cookies()",
  "response.text()",
  "response.body()",
  "storageState({ path:",
  "screenshot(",
  "tracing.start",
]) {
  requireAbsent(
    runner,
    forbidden,
    `browser harness must not retain sensitive browser material through ${forbidden}`,
  );
}

if (packageJson.devDependencies?.["@playwright/test"] === undefined) {
  throw new Error("Forum Page Builder harness must reuse the existing Playwright dependency");
}

for (const marker of [
  'required_permissions = ["forum_topics:read"]',
  'owner_data_state = "owner_property_editor_ready"',
  'preview_data_state = "owner_preview_transport_ready"',
]) {
  requireContains(moduleManifest, marker, `Forum contribution manifest missing ${marker}`);
}
for (const marker of [
  'data-page-builder-contribution-properties="true"',
  'id=input_id',
  'set_field("props", normalized_props.clone())',
]) {
  requireContains(propertyPanel, marker, `property panel missing browser boundary ${marker}`);
}
for (const marker of [
  'data-page-builder-contribution-preview="true"',
  'data-page-builder-contribution-preview-result="ready"',
]) {
  requireContains(previewPanel, marker, `preview panel missing browser boundary ${marker}`);
}
for (const marker of [
  "data-fly-block-id=browser_block_id",
  'data-fly-action="insert-block"',
  'data-fly-action="select-component"',
]) {
  requireContains(palette, marker, `palette/layer boundary missing ${marker}`);
}
for (const marker of [
  "with_preview_port(Arc::new(ForumPageBuilderPreviewPort))",
  "with_property_port(Arc::new(ForumPageBuilderPropertyPort))",
  'enabled_modules.contains("forum")',
]) {
  requireContains(composition, marker, `app composition missing ${marker}`);
}

for (const marker of [
  "Status: `source-ready / maintainer-browser-execution-pending / runtime-evidence-pending`",
  "forum_page_builder_browser_execution_v1",
  "browser_execution_passed_runtime_evidence_pending",
  "preview_off",
  "properties_off",
  "Forum-disabled",
  "forum_topics:read",
  "maintainer-supplied reviewed deployment identity",
  "No browser execution is claimed",
]) {
  requireContains(packet, marker, `browser evidence actualization missing ${marker}`);
}

console.log("Forum Page Builder browser evidence harness source: ok");
