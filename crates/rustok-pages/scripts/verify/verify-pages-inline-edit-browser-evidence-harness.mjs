#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  contract:
    "crates/rustok-pages/contracts/evidence/pages-inline-edit-browser-execution-contract.json",
  evidence:
    "crates/rustok-pages/contracts/evidence/pages-inline-edit-browser-evidence-harness-source.json",
  config: "apps/next-admin/playwright.pages-inline-edit.config.ts",
  test: "apps/next-admin/tests/pages-inline-edit/browser-evidence.spec.ts",
  package: "apps/next-admin/package.json",
  pageBuilderInline: "crates/rustok-page-builder-storefront/src/inline_edit.rs",
  pagesInline: "crates/rustok-pages/storefront/src/inline_edit.rs",
  adminLaunch: "crates/rustok-pages/admin/src/inline_edit_launch.rs",
  realDom: "crates/fly-leptos/src/real_dom_inline.rs",
  artifactContract:
    "crates/rustok-pages/contracts/evidence/pages-inline-edit-artifact-http-execution-contract.json",
  packet:
    "docs/modules/pages-page-builder-inline-edit-browser-evidence-harness-packet-2026-08-06.md",
  executionPlan: "docs/modules/pages-page-builder-inline-edit-execution-plan.md",
};

const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const exact = (actual, expected, label) => {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) failures.push(`${label} drifted`);
};

for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(absolute(relativePath))) {
    failures.push(`${label}: missing ${relativePath}`);
    continue;
  }
  const stats = fs.lstatSync(absolute(relativePath));
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${label}: ${relativePath} must be a regular non-symlink file`);
  }
}
if (failures.length > 0) {
  console.error("[verify-pages-inline-edit-browser-evidence-harness] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const contract = JSON.parse(read(files.contract));
const evidence = JSON.parse(read(files.evidence));
const artifactContract = JSON.parse(read(files.artifactContract));
const sources = Object.fromEntries(
  Object.entries(files)
    .filter(([key]) => !["contract", "evidence", "artifactContract"].includes(key))
    .map(([key, relativePath]) => [key, read(relativePath)]),
);

if (contract.schema_version !== 1) failures.push("contract schema_version must be 1");
if (contract.module !== "pages") failures.push("contract module must be pages");
if (contract.packet !== "pages-inline-edit-browser-execution") {
  failures.push("contract packet identity drifted");
}
if (contract.status !== "source_ready_maintainer_execution_pending") {
  failures.push("contract source status drifted");
}
if (contract.promotion_boundary !== "does_not_close_tenant_rollout_ffa_or_fba") {
  failures.push("contract promotion boundary drifted");
}
exact(
  contract.artifact_http_input,
  {
    environment: "RUSTOK_PAGES_INLINE_EDIT_BROWSER_ARTIFACT_HTTP_EVIDENCE",
    format: "pages_inline_edit_artifact_http_execution_v1",
    status: "artifact_http_execution_passed_browser_rollout_pending",
    same_source_commit_required: true,
    origin_must_match: true,
    deployment_digest_must_match: true,
  },
  "artifact/HTTP input contract",
);
if (
  contract.artifact_http_input.format !== artifactContract.output?.format ||
  contract.artifact_http_input.status !== artifactContract.output?.status
) {
  failures.push("browser artifact/HTTP input is not tied to the artifact/HTTP output contract");
}
exact(
  contract.playwright,
  {
    config: "apps/next-admin/playwright.pages-inline-edit.config.ts",
    test: "apps/next-admin/tests/pages-inline-edit/browser-evidence.spec.ts",
    project: "pages-inline-edit-chromium",
    browser: "chromium",
    workers: 1,
    retries: 0,
    trace: "off",
    screenshot: "off",
    video: "off",
  },
  "Playwright contract",
);
exact(
  contract.output,
  {
    environment: "RUSTOK_PAGES_INLINE_EDIT_BROWSER_OUTPUT",
    default_path: "target/pages-inline-edit-browser-evidence.json",
    format: "pages_inline_edit_browser_execution_v1",
    status: "browser_execution_passed_rollout_pending",
    atomic_replace: true,
    automatic_canonical_source_mutation: false,
  },
  "browser output contract",
);

const expectedScenarios = [
  "launch_visible_for_allowed_draft",
  "launch_hidden_for_published",
  "launch_hidden_for_locale_less",
  "launch_hidden_for_missing",
  "launch_hidden_for_unauthorized",
  "launch_hidden_for_standalone_admin",
  "launch_href_is_relative_same_origin_exact_locale",
  "ssr_and_hydrated_dom_exclude_session_grant_and_proof_markers",
  "dedicated_client_mounts_without_critical_failures",
  "only_static_leaf_component_is_editable",
  "single_focusout_emits_one_commit",
  "successful_save_replaces_revision_and_project_hash",
  "reload_retains_saved_text_and_revision",
  "second_preloaded_tab_is_rejected_as_stale_without_partial_write",
  "exact_successful_request_replay_is_rejected",
  "delayed_commit_is_rejected_after_expiry_without_partial_write",
];
exact(contract.scenarios, expectedScenarios, "browser scenario list");
for (const [key, value] of Object.entries(contract.fixture_contract ?? {})) {
  if (value !== true) failures.push(`fixture_contract.${key} must be true`);
}
for (const required of [
  files.contract,
  files.evidence,
  files.config,
  files.test,
  files.packet,
  files.executionPlan,
  files.artifactContract,
  files.pagesInline,
  files.adminLaunch,
  files.pageBuilderInline,
  files.realDom,
]) {
  if (!contract.required_source_files?.includes(required)) {
    failures.push(`required_source_files is missing ${required}`);
  }
}
for (const forbidden of [
  "authorization_header",
  "cookie_header",
  "storage_state_contents",
  "session_id",
  "authorization_proof",
  "grant",
  "raw_request_body",
  "raw_response_body",
  "raw_html",
  "console_message_text",
  "page_id",
  "component_id",
  "edited_text",
  "admin_path",
  "trace",
  "screenshot",
  "video",
]) {
  if (!contract.privacy_boundary?.forbidden_persisted_values?.includes(forbidden)) {
    failures.push(`privacy boundary is missing ${forbidden}`);
  }
}

if (evidence.format !== "pages_inline_edit_browser_evidence_harness_source_v1") {
  failures.push("source evidence format drifted");
}
if (evidence.status !== "pages_inline_edit_browser_evidence_harness_source_unvalidated") {
  failures.push("source evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("source evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`source evidence validation.${key} must remain false`);
}
for (const key of [
  "machine_browser_execution_contract_added",
  "existing_pinned_playwright_dependency_reused",
  "chromium_is_explicit",
  "single_worker_and_zero_retries_are_locked",
  "trace_screenshot_and_video_are_disabled",
  "artifact_http_evidence_is_required_first",
  "artifact_http_source_commit_origin_and_deployment_digest_must_match",
  "editor_unauthorized_and_standalone_storage_states_are_external_inputs",
  "storage_state_contents_are_not_persisted",
  "allowed_draft_launch_visibility_is_checked",
  "published_locale_less_missing_unauthorized_and_standalone_hidden_states_are_checked",
  "launch_href_is_relative_same_origin_and_exact_locale",
  "ssr_and_hydrated_dom_secret_marker_absence_is_checked",
  "hydrated_root_identity_is_bound_to_page_and_project_hash",
  "dedicated_authoring_assets_are_observed",
  "critical_request_failures_console_errors_and_page_errors_are_bounded",
  "editable_static_leaf_is_checked",
  "provider_composite_templated_interactive_and_runtime_owned_components_are_read_only",
  "one_changed_focusout_requires_one_commit_request",
  "replacement_revision_and_project_hash_are_checked",
  "reload_persistence_is_checked",
  "two_preloaded_tabs_produce_a_stale_rejection_without_partial_write",
  "exact_successful_request_replay_is_checked",
  "delayed_request_expiry_is_checked_without_partial_write",
  "raw_request_and_response_bodies_are_not_persisted",
  "raw_html_console_text_paths_page_ids_component_ids_and_edited_text_are_not_persisted",
  "output_is_atomically_replaced",
  "canonical_source_is_not_mutated_automatically",
  "tenant_rollout_remains_separate",
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`source_contract.${key} must be true`);
  }
}
for (const key of [
  "tests_run",
  "static_verifiers_run",
  "cargo_run",
  "npm_or_playwright_run",
  "browser_run",
  "http_requests_run",
  "database_or_fixture_setup_run",
  "workflows_or_ci_run",
  "artifact_http_execution_observed",
  "browser_execution_observed",
  "tenant_rollout_observed",
  "ffa_promoted",
  "fba_promoted",
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`source_contract.${key} must remain false`);
  }
}

for (const marker of [
  'testDir: "./tests/pages-inline-edit"',
  "fullyParallel: false",
  "forbidOnly: true",
  "retries: 0",
  "workers: 1",
  'trace: "off"',
  'screenshot: "off"',
  'video: "off"',
  'name: "pages-inline-edit-chromium"',
  'devices["Desktop Chrome"]',
  'reporter: [["list"]]',
]) need(sources.config, marker, "Playwright config");
for (const marker of [
  "trace: \"retain",
  "screenshot: \"only-on-failure",
  "video: \"retain",
  "retries: 1",
  "fullyParallel: true",
]) forbid(sources.config, marker, "Playwright config");
need(sources.package, '"@playwright/test": "^1.60.0"', "pinned browser dependency");

for (const marker of [
  "RUSTOK_PAGES_INLINE_EDIT_BROWSER_ARTIFACT_HTTP_EVIDENCE",
  "validateArtifactHttpInput(artifactInput.document, head, baseOrigin, deploymentDigest)",
  "document.source_commit !== head",
  "http?.origin !== origin",
  "http?.deployment_image_digest !== deploymentDigest",
  "docker.repo_digests.includes(deploymentDigest)",
  "RUSTOK_PAGES_INLINE_EDIT_BROWSER_EDITOR_STORAGE_STATE",
  "RUSTOK_PAGES_INLINE_EDIT_BROWSER_UNAUTHORIZED_STORAGE_STATE",
  "RUSTOK_PAGES_INLINE_EDIT_BROWSER_STANDALONE_STORAGE_STATE",
  "storageState: storageInputs.editor.path",
  "storageState: storageInputs.unauthorized.path",
  "storageState: storageInputs.standalone.path",
  "allowed launch must use a relative same-origin href",
  "target.searchParams.get(\"page_id\") !== pageId",
  "target.searchParams.get(\"lang\") !== locale",
  "assertLaunchHidden(editorContext, paths.published)",
  "assertLaunchHidden(editorContext, paths.localeLess)",
  "assertLaunchHidden(editorContext, paths.missing)",
  "assertLaunchHidden(unauthorizedContext, paths.draft)",
  "assertLaunchHidden(standaloneContext, paths.standalone)",
  "scanForbiddenDomMarkers(ssrHtml, \"authoring SSR HTML\")",
  "scanForbiddenDomMarkers(await mainPage.content(), \"hydrated authoring DOM\")",
  "root.getAttribute(\"data-inline-session\")",
  "root.getAttribute(\"data-inline-proof\")",
  "fly-inline-${domId(pageId)}-${projectHash}",
  "authoringAssetPaths.size",
  "mainFailures.console_errors !== 0",
  "mainFailures.page_errors !== 0",
  "mainFailures.critical_request_failures !== 0",
  "data-fly-inline-editable",
  "contenteditable",
  "captureCommit(mainPage, componentIds.editable, savedText)",
  "successful.requestCount !== 1",
  "currentRoot.revision === initialRoot.revision",
  "currentRoot.projectHash === initialRoot.projectHash",
  "replaySuccessfulRequest(editorContext, successful)",
  "replay.status() < 400",
  "captureRejectedCommit(stalePage",
  "partial document write",
  "waitForTimeout(expiryDelayMs)",
  "captureRejectedCommit(\n      expiryPage",
  "writeAtomic(output, outputDocument)",
  "storage_state_contents_persisted: false",
  "authorization_or_cookie_values_persisted: false",
  "session_ids_grants_or_proofs_persisted: false",
  "page_ids_component_ids_or_edited_text_persisted: false",
  "raw_html_persisted: false",
  "raw_request_or_response_bodies_persisted: false",
  "console_message_text_persisted: false",
  "traces_persisted: false",
  "screenshots_persisted: false",
  "videos_persisted: false",
  "tenant_rollout_executed: false",
  "ffa_promoted: false",
  "fba_promoted: false",
]) need(sources.test, marker, "browser evidence test");
for (const marker of [
  "writeFileSync(storageInputs",
  "JSON.stringify(storageInputs",
  "console_messages:",
  "raw_html:",
  "request_body:",
  "response_body:",
  "page_id: pageId",
  "component_id:",
  "edited_text:",
  "trace: \"on",
  "screenshot: \"on",
  "video: \"on",
]) forbid(sources.test, marker, "browser evidence privacy boundary");

for (const marker of [
  "let root_id = inline_root_id(&grant);",
  "dom_id(grant.page_id())",
  "grant.expected_project_hash().hex()",
  "inline_dom_identity_excludes_grant_session_and_authorization_proof",
]) need(sources.pageBuilderInline, marker, "session-free Page Builder DOM identity");
for (const marker of [
  "data-inline-session",
  "dom_id(grant.session_id())",
  "data-inline-proof",
]) forbid(sources.pageBuilderInline, marker, "session-free Page Builder DOM identity");
for (const marker of [
  'data-pages-inline-edit-launch="same-origin"',
  'PAGES_AUTHORING_PATH: &str = "/modules/pages-authoring"',
  "if page.status.eq_ignore_ascii_case(\"published\")",
]) need(sources.adminLaunch, marker, "admin launch source");
for (const marker of [
  'data-pages-authenticated-inline-edit="true"',
  "Inline edit saved.",
  "pages/inline-edit/bootstrap",
  "pages/inline-edit/commit",
]) need(sources.pagesInline, marker, "Pages inline consumer source");
for (const marker of [
  'FLY_REAL_DOM_COMPONENT_ATTRIBUTE: &str = "data-fly-component-id"',
  'FLY_REAL_DOM_INLINE_ATTRIBUTE: &str = "data-fly-inline-editable"',
  'set_attribute("contenteditable", "plaintext-only")',
  '"focusout"',
]) need(sources.realDom, marker, "real-DOM adapter source");

for (const marker of [
  "source-ready / maintainer-execution-pending",
  "browser_execution_passed_rollout_pending",
  "artifact/HTTP packet",
  "two preloaded tabs",
  "exact request replay",
  "short-lived grant",
  "storage-state contents are never copied",
  "No browser execution is claimed",
]) need(sources.packet, marker, "browser evidence packet");
for (const marker of [
  "inline-edit-browser-evidence-harness-source-ready",
  "verify-pages-inline-edit-browser-evidence-harness.mjs",
  "playwright.pages-inline-edit.config.ts",
  "browser_execution_passed_rollout_pending",
  "browser evidence harness: source-ready",
  "browser execution: pending",
]) need(sources.executionPlan, marker, "active execution plan");

if (failures.length > 0) {
  console.error("[verify-pages-inline-edit-browser-evidence-harness] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log(
  "[verify-pages-inline-edit-browser-evidence-harness] PASS browser_harness_source_ready=true execution=pending rollout=pending",
);
