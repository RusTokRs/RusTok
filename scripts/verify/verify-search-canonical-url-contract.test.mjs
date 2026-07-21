#!/usr/bin/env node

import test from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const verifier = path.resolve("scripts/verify/verify-search-canonical-url-contract.mjs");

function write(root, relativePath, content) {
  const target = path.join(root, relativePath);
  mkdirSync(path.dirname(target), { recursive: true });
  writeFileSync(target, content);
}

function fixture({ localGraphqlPolicy = false, missingBlogSourceCheck = false } = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-search-url-contract-"));
  write(
    root,
    "crates/rustok-search/src/engine.rs",
    `
      const BLOG_SOURCE_MODULE: &str = "blog";
      const BLOG_ENTITY_TYPE: &str = "blog_post";
      const BLOG_STOREFRONT_ROUTE: &str = "/modules/blog";
      const MAX_BLOG_SLUG_LEN: usize = 200;
      pub fn canonical_search_result_url(value: &SearchResultItem) -> Option<String> {
        ${missingBlogSourceCheck ? "true" : "value.source_module == BLOG_SOURCE_MODULE"};
        content_kind_query(&value.source_module);
        canonical_blog_result_url(&value.payload)
      }
      fn canonical_blog_result_url(payload: &serde_json::Value) -> Option<String> {
        let slug = payload.get("slug")?.as_str()?;
        let _ = MAX_BLOG_SLUG_LEN;
        let _ = slug.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'));
        Some(format!("{BLOG_STOREFRONT_ROUTE}?slug={slug}"))
      }
      fn content_kind_query(_: &str) -> String { String::new() }
    `,
  );
  write(
    root,
    "crates/rustok-search/src/lib.rs",
    "pub use engine::canonical_search_result_url;",
  );
  write(
    root,
    "crates/rustok-search/src/graphql/types.rs",
    localGraphqlPolicy
      ? "fn derive_search_result_url(_: &SearchResultItem) -> Option<String> { None }"
      : "let url = crate::canonical_search_result_url(&value);",
  );
  write(
    root,
    "crates/rustok-search/storefront/src/transport/navigation.rs",
    "item.url.is_some(); item.url = blog_result_url(item.payload.as_str()); preserves_backend_url_and_rejects_invalid_slug",
  );
  write(
    root,
    "crates/rustok-search/contracts/evidence/search-canonical-url-contract.json",
    JSON.stringify({
      schema_version: 1,
      module: "search",
      surface: "canonical_result_url",
      status: "source_verified_no_compile",
      compile_policy: "not_run_by_request",
      production_contract: {
        normalized_result: "crates/rustok-search/src/engine.rs",
        graphql_projection: "crates/rustok-search/src/graphql/types.rs",
        compatibility_fallback: "crates/rustok-search/storefront/src/transport/navigation.rs",
      },
    }),
  );
  write(
    root,
    "crates/rustok-search/docs/implementation-plan.md",
    "search-canonical-url-contract.json canonical_search_result_url compatibility fallback",
  );
  return root;
}

function run(root) {
  return spawnSync(process.execPath, [verifier], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

test("canonical Search URL verifier accepts the owner contract", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("canonical Search URL verifier rejects a transport-local GraphQL switch", () => {
  const root = fixture({ localGraphqlPolicy: true });
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /missing crate::canonical_search_result_url|forbidden fn derive_search_result_url/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("canonical Search URL verifier rejects missing Blog source ownership", () => {
  const root = fixture({ missingBlogSourceCheck: true });
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /missing value.source_module == BLOG_SOURCE_MODULE/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
