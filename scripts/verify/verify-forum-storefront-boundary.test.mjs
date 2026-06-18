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
pub fn forum_storefront_accent_style() {}
pub fn forum_storefront_status_badge_class() {}
`;
}

function uiSource({ rawAccent = false, rawHref = false, missingCoreUse = false } = {}) {
  return `${missingCoreUse ? "" : "use crate::core::{forum_storefront_category_card_view_model, forum_storefront_topic_card_view_model, forum_storefront_status_badge_class, forum_storefront_count_label};\n"}
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
  writeFixtureFile(root, "crates/rustok-forum/storefront/src/lib.rs", "pub use ui::leptos::ForumView;\n");
  writeFixtureFile(root, "crates/rustok-forum/storefront/src/core.rs", coreSource(options));
  writeFixtureFile(root, "crates/rustok-forum/storefront/src/ui/leptos.rs", uiSource(options));
  writeFixtureFile(root, "crates/rustok-forum/storefront/src/transport.rs", "pub async fn fetch_storefront_forum() {}\n");
  writeFixtureFile(root, "crates/rustok-forum/storefront/src/api.rs", "mod graphql {}\n");
  writeFixtureFile(root, "crates/rustok-forum/docs/implementation-plan.md", "verify-forum-storefront-boundary.mjs\n");
  writeFixtureFile(root, "docs/modules/registry.md", "verify-forum-storefront-boundary.mjs\n");
  writeFixtureFile(root, "scripts/verify/verify-forum-storefront-boundary.test.mjs", "passes canonical fixture\nrejects Leptos-specific core\n");
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

test("forum storefront boundary verifier rejects raw UI accent fallback", () => {
  const result = run(fixture({ rawAccent: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /category accent fallback must stay in core/);
});

test("forum storefront boundary verifier rejects missing package aggregate wiring", () => {
  const result = run(fixture({ omitAggregate: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /aggregate FFA verifier must include forum storefront boundary verifier/);
});
