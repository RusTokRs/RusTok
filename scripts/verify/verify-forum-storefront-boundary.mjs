#!/usr/bin/env node
// RusTok forum storefront FFA boundary guardrails.

import { existsSync, readFileSync } from "node:fs";

const files = {
  lib: "crates/rustok-forum/storefront/src/lib.rs",
  core: "crates/rustok-forum/storefront/src/core.rs",
  ui: "crates/rustok-forum/storefront/src/ui/leptos.rs",
  transport: "crates/rustok-forum/storefront/src/transport/mod.rs",
  graphqlAdapter: "crates/rustok-forum/storefront/src/transport/graphql_adapter.rs",
  legacyApi: "crates/rustok-forum/storefront/src/api.rs",
  plan: "crates/rustok-forum/docs/implementation-plan.md",
  registry: "docs/modules/registry.md",
  packageJson: "package.json",
  verifierTest: "scripts/verify/verify-forum-storefront-boundary.test.mjs",
};

function text(path) {
  try { return readFileSync(path, "utf8"); } catch (error) { fail(`${path}: ${error.message}`); }
}
function fail(message) { console.error("forum storefront boundary verification failed:"); console.error(`- ${message}`); process.exit(1); }
function assertContains(source, needle, message) { if (!source.includes(needle)) fail(message); }
function assertNotContains(source, needle, message) { if (source.includes(needle)) fail(message); }

const lib = text(files.lib);
const core = text(files.core);
const ui = text(files.ui);
const transport = text(files.transport);
const graphqlAdapter = text(files.graphqlAdapter);
const plan = text(files.plan);
const registry = text(files.registry);
const verifierTest = text(files.verifierTest);
const pkg = JSON.parse(text(files.packageJson));

[
  "forum_storefront_category_card_view_model",
  "forum_storefront_topic_card_view_model",
  "forum_storefront_count_label",
  "forum_storefront_slug_label",
  "forum_storefront_category_card_class",
  "forum_storefront_topic_card_class",
  "forum_storefront_accent_style",
  "forum_storefront_status_badge_class",
  "ForumStorefrontCategoryRailLabels",
].forEach((name) => assertContains(core, name, `${files.core}: missing core-owned storefront policy ${name}`));

assertNotContains(core, "leptos::", `${files.core}: core must remain framework-agnostic`);
assertNotContains(core, "view!", `${files.core}: core must not render Leptos views`);
assertNotContains(ui, "fn status_badge_class", `${files.ui}: status badge class policy must stay in core`);
assertNotContains(ui, "background:linear-gradient", `${files.ui}: category accent fallback must stay in core`);
assertNotContains(ui, "?category={category_id}", `${files.ui}: route href construction must stay in core`);
assertContains(ui, "forum_storefront_category_card_view_model", `${files.ui}: UI must consume core-owned category card view-model`);
assertContains(ui, "forum_storefront_topic_card_view_model", `${files.ui}: UI must consume core-owned topic card view-model`);
assertContains(ui, "forum_storefront_status_badge_class", `${files.ui}: UI must consume core-owned status badge class policy`);
assertContains(ui, "forum_storefront_count_label", `${files.ui}: UI must consume core-owned count label policy`);
assertContains(transport, "fetch_storefront_forum", `${files.transport}: storefront transport facade must expose fetch_storefront_forum`);
assertContains(transport, "mod graphql_adapter;", `${files.transport}: transport facade must own GraphQL adapter module`);
assertContains(transport, "graphql_adapter::fetch_storefront_forum", `${files.transport}: transport facade must delegate through GraphQL adapter`);
assertNotContains(transport, "crate::api", `${files.transport}: transport facade must not delegate to legacy api module`);
assertContains(graphqlAdapter, "GraphqlRequest", `${files.graphqlAdapter}: storefront GraphQL adapter must keep GraphQL-backed read contract`);
if (existsSync(files.legacyApi)) {
  fail(`${files.legacyApi}: legacy api.rs must stay removed; transport/graphql_adapter.rs owns the read contract`);
}
assertNotContains(lib, "mod api;", `${files.lib}: lib must not wire legacy api module`);
assertContains(lib, "pub use ui::leptos::ForumView", `${files.lib}: lib must only wire and re-export ForumView`);
assertContains(plan, "verify-forum-storefront-boundary.mjs", `${files.plan}: local plan must mention storefront fast boundary guardrail`);
assertContains(registry, "verify-forum-storefront-boundary.mjs", `${files.registry}: central readiness board must mention storefront fast boundary guardrail`);
assertContains(verifierTest, "passes canonical fixture", `${files.verifierTest}: verifier fixture tests must cover the canonical pass path`);
assertContains(verifierTest, "rejects Leptos-specific core", `${files.verifierTest}: verifier fixture tests must cover framework leakage`);

const scripts = pkg.scripts ?? {};
if (scripts["verify:forum:storefront-boundary"] !== "node scripts/verify/verify-forum-storefront-boundary.mjs") {
  fail(`${files.packageJson}: package scripts must expose forum storefront boundary verifier`);
}
if (!String(scripts["verify:ffa:ui:migration"] ?? "").includes("npm run verify:forum:storefront-boundary")) {
  fail(`${files.packageJson}: aggregate FFA verifier must include forum storefront boundary verifier`);
}
if (scripts["test:verify:forum:storefront-boundary"] !== "node scripts/verify/verify-forum-storefront-boundary.test.mjs") {
  fail(`${files.packageJson}: package scripts must expose forum storefront boundary fixture tests`);
}
if (!String(scripts["test:verify:ffa:ui:migration"] ?? "").includes("npm run test:verify:forum:storefront-boundary")) {
  fail(`${files.packageJson}: aggregate FFA fixture tests must include forum storefront boundary fixtures`);
}

console.log("forum storefront boundary verification passed");
