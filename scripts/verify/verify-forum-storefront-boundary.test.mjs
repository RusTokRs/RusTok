#!/usr/bin/env node
import test from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-forum-storefront-boundary.mjs");

function writeFixtureFile(root, filePath, contents) {
  const target = path.join(root, filePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, contents);
}

function coreSource({ leptosCore = false } = {}) {
  return `${leptosCore ? "use leptos::prelude::*;\n" : ""}
pub struct ForumStorefrontCategoryRailLabels;
pub fn forum_storefront_category_card_view_model() {}
pub fn forum_storefront_topic_card_view_model() {}
pub fn forum_storefront_count_label() {}
pub fn forum_storefront_slug_label() {}
pub fn forum_storefront_category_card_class() {}
pub fn forum_storefront_topic_card_class() {}
pub fn forum_storefront_accent_class() {}
pub fn forum_storefront_status_badge_class() {}
`;
}

function uiSource({
  rawAccent = false,
  rawHref = false,
  missingCoreUse = false,
  directRichtextHtml = false,
  missingBidi = false,
} = {}) {
  const categoryBidi = missingBidi ? "" : `
forum_storefront_content_lang(item.effective_locale.as_str());
let description_lang = if item { ui_content_lang.clone() };
data-forum-target-localized="";
lang=content_lang;
lang=description_lang;
dir="auto";
data-forum-route-identifier="";
dir="ltr";
`;
  return `${missingCoreUse ? "" : "use crate::core::{forum_storefront_category_card_view_model, forum_storefront_topic_card_view_model, forum_storefront_status_badge_class, forum_storefront_count_label};\n"}
use leptos_ui::RichTextHtml;
use rustok_api::normalize_locale_tag;
fn forum_storefront_content_lang(locale: &str) -> String { normalize_locale_tag(locale).unwrap_or_else(|| "und".to_string()) }
fn ForumCategoryRail() {
${categoryBidi}
}
fn ForumTopicFeed() {
forum_storefront_content_lang(card.effective_locale.as_str());
<span dir="ltr";
data-forum-target-localized="";
lang=content_lang;
dir="auto";
data-forum-route-identifier="";
dir="ltr";
}
fn ForumThreadPanel() {
forum_storefront_content_lang(topic.effective_locale.as_str());
<span dir="ltr";
data-forum-target-localized="";
lang=content_lang.clone();
dir="auto";
data-forum-route-identifier="";
dir="ltr";
<RichTextHtml view=body content_locale=body_locale />;
}
fn ReplyCard() {
<span dir="ltr";
<RichTextHtml view=content content_locale=content_locale />;
}
${directRichtextHtml ? "inner_html=body.html;" : "<RichTextHtml view=body content_locale=body_locale />;\n<RichTextHtml view=content content_locale=content_locale />;"}
${rawAccent ? 'const STYLE: &str = "background:linear-gradient(180deg,#0ea5e9 0%,#f59e0b 100%);";\n' : ""}
${rawHref ? 'const HREF: &str = "?category={category_id}";\n' : ""}
`;
}

function packageSource({ omitVerify = false, omitAggregate = false } = {}) {
  return JSON.stringify({
    scripts: {
      ...(omitVerify ? {} : { "verify:forum:storefront-boundary": "node scripts/verify/verify-forum-storefront-boundary.mjs" }),
      "test:verify:forum:storefront-boundary": "node scripts/verify/verify-forum-storefront-boundary.test.mjs",
      "test:verify:ffa:ui:migration": "npm run test:verify:forum:admin-boundary && npm run test:verify:forum:storefront-boundary",
      "verify:ffa:ui:migration": omitAggregate
        ? "npm run verify:forum:admin-boundary"
        : "npm run verify:forum:admin-boundary && npm run verify:forum:storefront-boundary",
    },
  });
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-forum-storefront-boundary-"));
  writeFixtureFile(root, "crates/rustok-forum/storefront/src/lib.rs", `${options.restoredApi ? "mod api;" : ""}\npub use ui::leptos::ForumView;\n`);
  writeFixtureFile(root, "crates/rustok-forum/storefront/src/core.rs", coreSource(options));
  writeFixtureFile(root, "crates/rustok-forum/storefront/src/ui/leptos.rs", uiSource(options));
  writeFixtureFile(root, "crates/rustok-forum/storefront/src/transport/mod.rs", "mod graphql_adapter { include!(\"graphql_adapter.rs\"); }\npub async fn fetch_storefront_forum() { graphql_adapter::fetch_storefront_forum().await; }\n");
  writeFixtureFile(root, "crates/rustok-forum/storefront/src/transport/graphql_adapter.rs", "use rustok_graphql::GraphqlRequest;\npub async fn fetch_storefront_forum() {}\n");
  writeFixtureFile(root, "crates/rustok-forum/storefront/Cargo.toml", "[dependencies]\nleptos-ui.workspace = true\n");
  if (options.restoredApi) writeFixtureFile(root, "crates/rustok-forum/storefront/src/api.rs", "mod graphql {}\n");
  writeFixtureFile(root, "crates/rustok-forum/docs/implementation-plan.md", "verify-forum-storefront-boundary.mjs shared `RichTextHtml`\n");
  writeFixtureFile(root, "docs/modules/registry.md", "verify-forum-storefront-boundary.mjs\n");
  writeFixtureFile(root, "scripts/verify/verify-forum-storefront-boundary.test.mjs", "passes canonical fixture\nrejects Leptos-specific core\nrejects direct richtext HTML rendering\nrejects missing storefront content-locale bidi\n");
  writeFixtureFile(root, "package.json", packageSource(options));
  return root;
}

function run(root) {
  return spawnSync(process.execPath, [scriptPath], { cwd: root, encoding: "utf8" });
}

test("forum storefront boundary verifier passes canonical fixture", () => {
  const result = run(fixture());
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /forum storefront boundary verification passed/);
});

test("forum storefront boundary verifier rejects Leptos-specific core", () => {
  const result = run(fixture({ leptosCore: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /core must remain framework-agnostic/);
});

test("forum storefront boundary verifier rejects direct richtext HTML rendering", () => {
  const result = run(fixture({ directRichtextHtml: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /topic projection must use RichTextHtml|must not bypass the shared RichTextHtml sink/);
});

test("forum storefront boundary verifier rejects missing storefront content-locale bidi", () => {
  const result = run(fixture({ missingBidi: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /category rail missing content-locale bidi marker/);
});

test("forum storefront boundary verifier rejects raw UI accent fallback", () => {
  const result = run(fixture({ rawAccent: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /inline category accent style must stay absent/);
});

test("forum storefront boundary verifier rejects missing package aggregate wiring", () => {
  const result = run(fixture({ omitAggregate: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /aggregate FFA verifier must include forum storefront boundary verifier/);
});

test("forum storefront boundary verifier rejects restored api module", () => {
  const result = run(fixture({ restoredApi: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /removed api\.rs must stay absent/);
});
