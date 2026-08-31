#!/usr/bin/env node

import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

function need(text, marker, label) {
  if (!text.includes(marker)) throw new Error(`${label}: missing ${marker}`);
}

function forbid(text, marker, label) {
  if (text.includes(marker)) throw new Error(`${label}: forbidden ${marker}`);
}

const testPath =
  "apps/next-admin/tests/forum-category-taxonomy/browser-evidence.spec.ts";
const configPath = "apps/next-admin/playwright.forum-category-taxonomy.config.ts";
const contractPath =
  "crates/rustok-forum/contracts/evidence/forum-category-taxonomy-browser-execution-contract.json";
const workflowPath = ".github/workflows/forum-category-taxonomy-browser-evidence.yml";
const testSource = read(testPath);
const config = read(configPath);
const contract = JSON.parse(read(contractPath));
const workflow = read(workflowPath);
const packageJson = JSON.parse(read("apps/next-admin/package.json"));
const adminRoot = read("crates/rustok-forum/admin/src/ui/root.rs");
const adminUi = read("crates/rustok-forum/admin/src/ui/category_dnd.rs");
const storefrontUi = read("crates/rustok-forum/storefront/src/ui/leptos.rs");
const treeOwner = read("crates/rustok-forum/src/services/category_taxonomy_tree_read.rs");
const routeMount = read("apps/storefront/src/forum_category_route.rs");

if (contract.status !== "source_ready_maintainer_execution_pending") {
  throw new Error("CAT-5 browser contract must not claim execution");
}
if (contract.runner !== testPath || contract.config !== configPath) {
  throw new Error("CAT-5 browser contract must point to the retained Playwright source");
}
if (contract.workflow !== workflowPath) {
  throw new Error("CAT-5 browser contract must point to the retained manual execution workflow");
}
for (const pending of [
  "browser execution",
  "deployment provenance",
  "production rollout completion",
  "TAXONOMY-CAT-5 completion",
]) {
  if (!contract.not_claimed?.includes(pending)) {
    throw new Error(`CAT-5 browser contract must keep ${pending} pending`);
  }
}
for (const claim of [
  "admin Category tree renders RTL Taxonomy-owned copy with effective locale, dir=auto and browser-computed direction=rtl",
  "storefront Category rail renders the same RTL Taxonomy-owned copy with browser-computed direction=rtl and canonical localized hrefs",
]) {
  if (!contract.claims_after_successful_execution?.includes(claim)) {
    throw new Error(`CAT-5 browser contract must retain computed RTL claim: ${claim}`);
  }
}
const mainOnlyDispatchBoundary =
  "mounted workflow_dispatch evidence fails closed unless the selected GitHub ref is refs/heads/main";
if (!contract.boundaries?.includes(mainOnlyDispatchBoundary)) {
  throw new Error("CAT-5 browser contract must retain the main-only mounted execution boundary");
}
const scopedAdminStateSecretBoundary =
  "raw authenticated admin storage state is scoped only to the late materialization step; the credential file is created after source verification and browser setup, only the Playwright execution step receives its path through a step output, and cleanup runs always";
if (!contract.boundaries?.includes(scopedAdminStateSecretBoundary)) {
  throw new Error("CAT-5 browser contract must retain the bounded admin storage-state lifetime boundary");
}
const mountedUrlPreflightBoundary =
  "mounted fixture URLs are preflighted before authenticated storage-state materialization and must be credential-free HTTP(S) URLs without fragments";
if (!contract.boundaries?.includes(mountedUrlPreflightBoundary)) {
  throw new Error("CAT-5 browser contract must retain the pre-auth mounted URL validation boundary");
}
const focusedPathClosureBoundary =
  "pull-request path filters cover every retained CAT-5 verifier input plus the next-admin package manifests so guarded source drift always runs the focused source contract";
if (!contract.boundaries?.includes(focusedPathClosureBoundary)) {
  throw new Error("CAT-5 browser contract must retain focused pull-request path closure");
}

for (const marker of contract.required_environment ?? []) {
  need(testSource, marker, "CAT-5 browser runner");
}

for (const marker of [
  "data-forum-target-localized",
  "data-forum-route-identifier",
  "toHaveAttribute('dir', 'auto')",
  "toHaveAttribute('dir', 'ltr')",
  "toHaveCSS('direction', 'rtl')",
  "depth 0 · position 0",
  "depth 1 · position 0",
  "allTextContents()",
  "RUSTOK_FORUM_CATEGORY_E2E_ACCENT_CLASS",
  "page.url()",
  "browser.newContext({ storageState })",
]) {
  need(testSource, marker, "CAT-5 browser runner");
}
for (const marker of [
  "function requiredUrl(name: string): string",
  "!['http:', 'https:'].includes(parsed.protocol)",
  "parsed.username",
  "parsed.password",
  "parsed.hash",
  "must be a credential-free HTTP(S) URL without a fragment",
]) {
  need(testSource, marker, "CAT-5 browser runner URL validation");
}
for (const forbidden of [
  "GraphqlRequest",
  "graphql",
  "request.post",
  "request.get",
  "page.evaluate",
  "screenshot(",
  "tracing.start",
]) {
  forbid(
    testSource,
    forbidden,
    "CAT-5 browser runner must observe mounted UI instead of bypassing transport",
  );
}

for (const marker of [
  "testDir: './tests/forum-category-taxonomy'",
  "fullyParallel: false",
  "retries: 0",
  "workers: 1",
  "trace: 'off'",
  "screenshot: 'off'",
  "video: 'off'",
  "forum-category-taxonomy-chromium",
]) {
  need(config, marker, "CAT-5 Playwright config");
}
if (packageJson.devDependencies?.["@playwright/test"] === undefined) {
  throw new Error("CAT-5 browser evidence must reuse the existing Playwright dependency");
}

for (const marker of [
  "workflow_dispatch:",
  "type: environment",
  "if: github.event_name == 'workflow_dispatch'",
  "environment: ${{ inputs.target_environment }}",
  "run: test \"$GITHUB_REF\" = \"refs/heads/main\"",
  "id: admin-storage-state",
  "state=\"$RUNNER_TEMP/forum-category-admin-storage-state.json\"",
  "echo \"path=$state\" >> \"$GITHUB_OUTPUT\"",
  "RUSTOK_FORUM_CATEGORY_ADMIN_STORAGE_STATE: ${{ steps.admin-storage-state.outputs.path }}",
  "npm ci --no-audit --no-fund",
  "npx --no-install playwright install --with-deps chromium",
  "npx --no-install playwright test --config=playwright.forum-category-taxonomy.config.ts --list",
  "npx --no-install playwright test --config=playwright.forum-category-taxonomy.config.ts",
  "rm -f \"$RUNNER_TEMP/forum-category-admin-storage-state.json\"",
]) {
  need(workflow, marker, "CAT-5 manual browser execution workflow");
}
const pullRequestPathsStart = workflow.indexOf("  pull_request:\n    paths:\n");
const workflowDispatchStart = workflow.indexOf("  workflow_dispatch:", pullRequestPathsStart);
if (pullRequestPathsStart < 0 || workflowDispatchStart <= pullRequestPathsStart) {
  throw new Error("CAT-5 workflow must retain an explicit pull_request path filter before workflow_dispatch");
}
const pullRequestPathsBlock = workflow.slice(pullRequestPathsStart, workflowDispatchStart);
for (const path of [
  workflowPath,
  "apps/next-admin/package.json",
  "apps/next-admin/package-lock.json",
  configPath,
  testPath,
  "apps/storefront/src/forum_category_route.rs",
  "crates/rustok-forum/admin/src/ui/root.rs",
  "crates/rustok-forum/admin/src/ui/category_dnd.rs",
  "crates/rustok-forum/storefront/src/ui/leptos.rs",
  "crates/rustok-forum/src/services/category_taxonomy_tree_read.rs",
  contractPath,
  "crates/rustok-forum/docs/cat5-category-taxonomy-browser-parity.md",
  "scripts/verify/verify-forum-category-taxonomy-browser-evidence.mjs",
]) {
  need(
    pullRequestPathsBlock,
    `      - "${path}"`,
    "CAT-5 focused workflow pull-request path closure",
  );
}
const adminStateSecretBinding =
  "ADMIN_STORAGE_STATE_JSON: ${{ secrets.RUSTOK_FORUM_CATEGORY_ADMIN_STORAGE_STATE_JSON }}";
const materializationSecretBlock = [
  "      - name: Materialize authenticated admin storage state",
  "        id: admin-storage-state",
  "        shell: bash",
  "        env:",
  `          ${adminStateSecretBinding}`,
  "        run: |",
  "          set -euo pipefail",
  '          test -n "$ADMIN_STORAGE_STATE_JSON"',
].join("\n");
need(
  workflow,
  materializationSecretBlock,
  "CAT-5 manual browser execution workflow step-scoped admin storage-state secret",
);
if (workflow.split(adminStateSecretBinding).length - 1 !== 1) {
  throw new Error("CAT-5 admin storage-state secret must be bound exactly once");
}
const storageStatePathBinding =
  "RUSTOK_FORUM_CATEGORY_ADMIN_STORAGE_STATE: ${{ steps.admin-storage-state.outputs.path }}";
if (workflow.split(storageStatePathBinding).length - 1 !== 1) {
  throw new Error("CAT-5 admin storage-state file path must be exposed only to the browser execution step");
}
forbid(
  workflow,
  "environment: ${{ inputs.target_environment }}\n    env:\n      ADMIN_STORAGE_STATE_JSON:",
  "CAT-5 admin storage-state secret must not be exposed at mounted job scope",
);
forbid(
  workflow,
  'echo "RUSTOK_FORUM_CATEGORY_ADMIN_STORAGE_STATE=$state" >> "$GITHUB_ENV"',
  "CAT-5 admin storage-state path must not be exported job-wide",
);
const mountedJobStart = workflow.indexOf("  mounted-browser-evidence:");
const mountedStepIndex = (marker) => workflow.indexOf(marker, mountedJobStart);
const mountedFixtureValidationIndex = mountedStepIndex(
  "      - name: Validate configured mounted fixture inputs",
);
const mountedVerifierIndex = mountedStepIndex("      - name: Verify CAT-5 browser evidence source contract");
const mountedInstallIndex = mountedStepIndex("      - name: Install next-admin dependencies");
const mountedChromiumIndex = mountedStepIndex("      - name: Install Chromium");
const mountedMaterializeIndex = mountedStepIndex("      - name: Materialize authenticated admin storage state");
const mountedExecuteIndex = mountedStepIndex("      - name: Execute mounted multilingual RTL browser evidence");
const mountedCleanupIndex = mountedStepIndex("      - name: Remove authenticated admin storage state");
if (
  mountedJobStart < 0 ||
  mountedFixtureValidationIndex < 0 ||
  mountedVerifierIndex < 0 ||
  mountedInstallIndex < 0 ||
  mountedChromiumIndex < 0 ||
  mountedMaterializeIndex < 0 ||
  mountedExecuteIndex < 0 ||
  mountedCleanupIndex < 0 ||
  !(
    mountedFixtureValidationIndex < mountedVerifierIndex &&
    mountedVerifierIndex < mountedInstallIndex &&
    mountedInstallIndex < mountedChromiumIndex &&
    mountedChromiumIndex < mountedMaterializeIndex &&
    mountedMaterializeIndex < mountedExecuteIndex &&
    mountedExecuteIndex < mountedCleanupIndex
  )
) {
  throw new Error(
    "CAT-5 mounted fixture validation and source/dependency/browser setup must complete before authenticated admin storage-state materialization and browser execution",
  );
}
const mountedFixtureValidationBlock = workflow.slice(
  mountedFixtureValidationIndex,
  mountedVerifierIndex,
);
for (const marker of [
  "node <<'NODE'",
  "const urlNames = [",
  "new URL(raw)",
  '!["http:", "https:"].includes(parsed.protocol)',
  "parsed.username",
  "parsed.password",
  "parsed.hash",
  "must be a credential-free HTTP(S) URL without a fragment",
]) {
  need(
    mountedFixtureValidationBlock,
    marker,
    "CAT-5 mounted fixture URL preflight before authenticated state materialization",
  );
}
const mountedUrlEnvironmentNames = (contract.required_environment ?? []).filter((name) =>
  name.endsWith("_E2E_URL"),
);
if (mountedUrlEnvironmentNames.length !== 6) {
  throw new Error("CAT-5 browser contract must retain the six mounted browser URL inputs");
}
for (const name of mountedUrlEnvironmentNames) {
  need(
    mountedFixtureValidationBlock,
    `"${name}"`,
    "CAT-5 mounted fixture URL preflight coverage",
  );
}
forbid(
  mountedFixtureValidationBlock,
  "ADMIN_STORAGE_STATE_JSON",
  "CAT-5 mounted URL validation must complete before the authenticated state secret is accessed",
);
for (const marker of (contract.required_environment ?? []).filter(
  (name) => name !== "RUSTOK_FORUM_CATEGORY_ADMIN_STORAGE_STATE",
)) {
  need(
    workflow,
    marker + ": ${{ vars." + marker + " }}",
    "CAT-5 manual browser execution workflow environment",
  );
}
for (const forbidden of [
  "pull_request_target:",
  "contents: write",
  "RUSTOK_FORUM_CATEGORY_ADMIN_STORAGE_STATE_JSON: ${{ vars.",
]) {
  forbid(workflow, forbidden, "CAT-5 manual browser execution workflow security boundary");
}

for (const marker of [
  "fn forum_category_route_locale",
  'segments.next()? != "categories"',
  "normalize_locale_tag(locale)",
  "localized_route_context.locale = Some(route_locale)",
  "provide_context(localized_route_context)",
  'forum_category_route_locale(Some("categories/ar_sa"))',
]) {
  need(adminRoot, marker, "Forum admin Category locale-addressable mounted route");
}
for (const marker of [
  'data-forum-target-localized=""',
  'lang=content_lang.clone()',
  'dir="auto"',
  'data-forum-route-identifier=""',
  'dir="ltr"',
  "vm.effective_locale",
  "item.depth",
  "item.position",
  "vm.icon_label",
]) {
  need(adminUi, marker, "Forum admin Category mounted UI");
}
for (const marker of [
  'data-forum-target-localized=""',
  'lang=content_lang',
  'dir="auto"',
  'data-forum-route-identifier=""',
  'dir="ltr"',
  "item.effective_locale",
  "card.href",
  "card.accent_class",
]) {
  need(storefrontUi, marker, "Forum storefront Category mounted UI");
}

for (const marker of [
  "TaxonomyOwnerCategoryReader",
  "load_scoped_categories",
  "requested_locale",
  "effective_locale",
  "category.position",
  "category.icon_key",
  "category.color",
]) {
  need(treeOwner, marker, "Taxonomy-backed Forum Category tree owner");
}
for (const marker of [
  "resolve_storefront_category_route",
  "StorefrontForumCategoryRouteDisposition::Redirect",
  "canonical.path",
  "private_permanent_redirect",
]) {
  need(routeMount, marker, "mounted Forum Category canonical route");
}

for (const legacy of [
  "forum_category_translations",
  "forum_category_route_aliases",
  "ForumCategoryTranslationTargetProvider",
]) {
  forbid(treeOwner, legacy, "Taxonomy-backed Forum Category tree owner");
  forbid(routeMount, legacy, "mounted Forum Category canonical route");
}

console.log("Forum Category Taxonomy multilingual/RTL browser evidence source and manual execution workflow: ok");
