#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  contract:
    "crates/modules/rustok-pages/contracts/evidence/pages-inline-edit-browser-execution-contract.json",
  evidence:
    "crates/modules/rustok-pages/contracts/evidence/pages-inline-edit-browser-evidence-harness-source.json",
  config: "apps/next-admin/playwright.pages-inline-edit.config.ts",
  test: "apps/next-admin/tests/pages-inline-edit/browser-evidence.spec.ts",
  package: "apps/next-admin/package.json",
  pageBuilderInline: "crates/modules/rustok-page-builder/storefront/src/inline_edit.rs",
  pagesInline: "crates/modules/rustok-pages/storefront/src/inline_edit.rs",
  adminLaunch: "crates/modules/rustok-pages/admin/src/inline_edit_launch.rs",
  realDom: "crates/ui/fly/web/src/real_dom_inline.rs",
  artifactContract:
    "crates/modules/rustok-pages/contracts/evidence/pages-inline-edit-artifact-http-execution-contract.json",
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
, marker, "browser evidence packet");
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
