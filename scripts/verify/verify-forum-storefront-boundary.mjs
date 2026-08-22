#!/usr/bin/env node
// RusTok forum storefront FFA boundary guardrails.

import { existsSync, readFileSync } from "node:fs";

const files = {
  lib: "crates/rustok-forum/storefront/src/lib.rs",
  core: "crates/rustok-forum/storefront/src/core.rs",
  ui: "crates/rustok-forum/storefront/src/ui/leptos.rs",
  transport: "crates/rustok-forum/storefront/src/transport/mod.rs",
  graphqlAdapter: "crates/rustok-forum/storefront/src/transport/graphql_adapter.rs",
  cargo: "crates/rustok-forum/storefront/Cargo.toml",
  removedApi: "crates/rustok-forum/storefront/src/api.rs",
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
function sourceSlice(source, startMarker, endMarker, description) {
  const start = source.indexOf(startMarker);
  const end = source.indexOf(endMarker, start + startMarker.length);
  if (start < 0 || end < 0) fail(`${files.ui}: missing ${description} source boundary`);
  return source.slice(start, end);
}

const lib = text(files.lib);
const core = text(files.core);
const ui = text(files.ui);
const transport = text(files.transport);
const graphqlAdapter = text(files.graphqlAdapter);
const cargo = text(files.cargo);
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
  "forum_storefront_accent_class",
  "forum_storefront_status_badge_class",
  "ForumStorefrontCategoryRailLabels",
].forEach((name) => assertContains(core, name, `${files.core}: missing core-owned storefront policy ${name}`));

assertNotContains(core, "leptos::", `${files.core}: core must remain framework-agnostic`);
assertNotContains(core, "view!", `${files.core}: core must not render Leptos views`);
assertNotContains(ui, "fn status_badge_class", `${files.ui}: status badge class policy must stay in core`);
assertNotContains(ui, "background:linear-gradient", `${files.ui}: inline category accent style must stay absent`);
assertNotContains(ui, "?category={category_id}", `${files.ui}: route href construction must stay in core`);
assertContains(ui, "forum_storefront_category_card_view_model", `${files.ui}: UI must consume core-owned category card view-model`);
assertContains(ui, "forum_storefront_topic_card_view_model", `${files.ui}: UI must consume core-owned topic card view-model`);
assertContains(ui, "forum_storefront_status_badge_class", `${files.ui}: UI must consume core-owned status badge class policy`);
assertContains(ui, "forum_storefront_count_label", `${files.ui}: UI must consume core-owned count label policy`);
assertContains(ui, "use leptos_ui::RichTextHtml;", `${files.ui}: UI must use the shared server-projection renderer`);
assertContains(ui, "view=body", `${files.ui}: topic projection must use RichTextHtml`);
assertContains(ui, "content_locale=body_locale", `${files.ui}: topic projection must keep the effective locale`);
assertContains(ui, "view=content", `${files.ui}: reply projection must use RichTextHtml`);
assertContains(ui, "content_locale=content_locale", `${files.ui}: reply projection must keep the effective locale`);
assertNotContains(ui, "inner_html=", `${files.ui}: owner UI must not bypass the shared RichTextHtml sink`);
assertContains(ui, "use rustok_api::normalize_locale_tag;", `${files.ui}: storefront plain-text locale semantics must use the canonical locale normalizer`);
assertContains(ui, "fn forum_storefront_content_lang(locale: &str) -> String", `${files.ui}: storefront must expose a canonical plain-text content language helper`);

const categoryRail = sourceSlice(ui, "fn ForumCategoryRail(", "fn ForumTopicFeed(", "category rail");
[
  "forum_storefront_content_lang(item.effective_locale.as_str())",
  "let description_lang = if item",
  "ui_content_lang.clone()",
  "data-forum-target-localized=\"\"",
  "lang=content_lang",
  "lang=description_lang",
  "dir=\"auto\"",
  "data-forum-route-identifier=\"\"",
  "dir=\"ltr\"",
].forEach((marker) => assertContains(categoryRail, marker, `${files.ui}: category rail missing content-locale bidi marker ${marker}`));

const topicFeed = sourceSlice(ui, "fn ForumTopicFeed(", "fn ForumThreadPanel(", "topic feed");
[
  "forum_storefront_content_lang(card.effective_locale.as_str())",
  "<span dir=\"ltr\"",
  "data-forum-target-localized=\"\"",
  "lang=content_lang",
  "dir=\"auto\"",
  "data-forum-route-identifier=\"\"",
  "dir=\"ltr\"",
].forEach((marker) => assertContains(topicFeed, marker, `${files.ui}: topic feed missing content-locale bidi marker ${marker}`));

const threadPanel = sourceSlice(ui, "fn ForumThreadPanel(", "fn ReplyCard(", "thread panel");
[
  "forum_storefront_content_lang(topic.effective_locale.as_str())",
  "<span dir=\"ltr\"",
  "data-forum-target-localized=\"\"",
  "lang=content_lang.clone()",
  "dir=\"auto\"",
  "data-forum-route-identifier=\"\"",
  "dir=\"ltr\"",
  "content_locale=body_locale",
].forEach((marker) => assertContains(threadPanel, marker, `${files.ui}: thread panel missing content-locale bidi marker ${marker}`));

const replyCardStart = ui.indexOf("fn ReplyCard(");
if (replyCardStart < 0) fail(`${files.ui}: missing reply card source boundary`);
const replyCard = ui.slice(replyCardStart);
assertContains(replyCard, "<span dir=\"ltr\"", `${files.ui}: reply locale identifier must stay LTR`);
assertContains(replyCard, "content_locale=content_locale", `${files.ui}: reply projection must keep its effective locale`);

assertContains(cargo, "leptos-ui.workspace = true", `${files.cargo}: storefront must depend on the shared Leptos richtext view boundary`);
assertContains(transport, "fetch_storefront_forum", `${files.transport}: storefront transport facade must expose fetch_storefront_forum`);
assertContains(transport, "mod graphql_adapter {", `${files.transport}: transport facade must own the canonical GraphQL adapter module`);
assertContains(transport, "graphql_adapter::fetch_storefront_forum", `${files.transport}: transport facade must delegate through GraphQL adapter`);
assertNotContains(transport, "crate::api", `${files.transport}: transport facade must not delegate to the removed api module`);
assertContains(graphqlAdapter, "GraphqlRequest", `${files.graphqlAdapter}: storefront GraphQL adapter must keep GraphQL-backed read contract`);
if (existsSync(files.removedApi)) {
  fail(`${files.removedApi}: removed api.rs must stay absent; transport/graphql_adapter.rs owns the read contract`);
}
assertNotContains(lib, "mod api;", `${files.lib}: lib must not wire the removed api module`);
assertContains(lib, "pub use ui::leptos::ForumView", `${files.lib}: lib must only wire and re-export ForumView`);
assertContains(plan, "verify-forum-storefront-boundary.mjs", `${files.plan}: local plan must mention storefront fast boundary guardrail`);
assertContains(plan, "shared `RichTextHtml`", `${files.plan}: local plan must record the shared read-only richtext boundary`);
assertContains(registry, "verify-forum-storefront-boundary.mjs", `${files.registry}: central readiness board must mention storefront fast boundary guardrail`);
assertContains(verifierTest, "passes canonical fixture", `${files.verifierTest}: verifier fixture tests must cover the canonical pass path`);
assertContains(verifierTest, "rejects Leptos-specific core", `${files.verifierTest}: verifier fixture tests must cover framework leakage`);
assertContains(verifierTest, "rejects direct richtext HTML rendering", `${files.verifierTest}: verifier fixture tests must cover shared richtext rendering`);
assertContains(verifierTest, "rejects missing storefront content-locale bidi", `${files.verifierTest}: verifier fixture tests must cover storefront bidi semantics`);

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
  fail(`${files.packageJson}: aggregate FFA fixture tests must include forum boundary fixtures`);
}

console.log("forum storefront boundary verification passed");
