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
const testSource = read(testPath);
const config = read(configPath);
const contract = JSON.parse(read(contractPath));
const packageJson = JSON.parse(read("apps/next-admin/package.json"));
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

for (const marker of contract.required_environment ?? []) {
  need(testSource, marker, "CAT-5 browser runner");
}

for (const marker of [
  "data-forum-target-localized",
  "data-forum-route-identifier",
  "toHaveAttribute('dir', 'auto')",
  "toHaveAttribute('dir', 'ltr')",
  "depth 0 · position 0",
  "depth 1 · position 0",
  "allTextContents()",
  "RUSTOK_FORUM_CATEGORY_E2E_ACCENT_CLASS",
  "page.url()",
  "browser.newContext({ storageState })",
]) {
  need(testSource, marker, "CAT-5 browser runner");
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

console.log("Forum Category Taxonomy multilingual/RTL browser evidence source: ok");
