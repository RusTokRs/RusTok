#!/usr/bin/env node
import test from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const verifier = path.resolve("scripts/verify/verify-search-blog-navigation.mjs");

function write(root, relative, content) {
  const target = path.join(root, relative);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function fixture({ missingParity = false, unsafeSlug = false } = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-search-blog-navigation-"));
  write(
    root,
    "crates/rustok-search/storefront/src/transport/mod.rs",
    missingParity
      ? "execute_selected_transport("
      : "mod navigation; execute_selected_transport( navigation::enrich_search_result_urls(&mut payload);",
  );
  write(
    root,
    "crates/rustok-search/storefront/src/transport/navigation.rs",
    unsafeSlug
      ? `const BLOG_SOURCE_MODULE: &str = "blog";
const BLOG_ENTITY_TYPE: &str = "blog_post";
const BLOG_STOREFRONT_ROUTE: &str = "/modules/blog";
item.url.is_some(); serde_json::from_str(payload); value.get("slug");
Some(format!("{BLOG_STOREFRONT_ROUTE}?slug={slug}"));
preserves_backend_url_and_rejects_invalid_slug;`
      : `const BLOG_SOURCE_MODULE: &str = "blog";
const BLOG_ENTITY_TYPE: &str = "blog_post";
const BLOG_STOREFRONT_ROUTE: &str = "/modules/blog";
item.url.is_some(); serde_json::from_str(payload); value.get("slug"); valid_blog_slug;
Some(format!("{BLOG_STOREFRONT_ROUTE}?slug={slug}"));
preserves_backend_url_and_rejects_invalid_slug;`,
  );
  write(
    root,
    "crates/rustok-search/src/blog_projector.rs",
    `const BLOG_ENTITY_TYPE: &str = "blog_post";
const BLOG_SOURCE_MODULE: &str = "blog";
"slug": p.slug`,
  );
  write(
    root,
    "crates/rustok-search/storefront/src/model.rs",
    "pub struct SearchPreviewResultItem { pub url: Option<String>, pub payload: String }",
  );
  return root;
}

function run(root) {
  return spawnSync(process.execPath, [verifier], { cwd: root, encoding: "utf8" });
}

test("search blog navigation verifier passes canonical fixture", () => {
  const result = run(fixture());
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /search blog navigation verification passed/);
});

test("search blog navigation verifier rejects missing transport parity", () => {
  const result = run(fixture({ missingParity: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /transport parity/);
});

test("search blog navigation verifier rejects missing slug validation", () => {
  const result = run(fixture({ unsafeSlug: true }));
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /valid_blog_slug/);
});
