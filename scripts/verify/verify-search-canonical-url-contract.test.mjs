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

function fixture({
  localGraphqlPolicy = false,
  missingBlogSourceCheck = false,
  missingStorefrontNativeDelegation = false,
  missingAdminNativeDelegation = false,
  missingAdminShellDelegation = false,
  transportFallbackRegression = false,
  staleEvidenceFallback = false,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-search-url-contract-"));
  const enginePath = "crates/rustok-search/src/engine.rs";
  const graphqlPath = "crates/rustok-search/src/graphql/types.rs";
  const storefrontNativePath =
    "crates/rustok-search/storefront/src/transport/native_server_adapter.rs";
  const storefrontFacadePath = "crates/rustok-search/storefront/src/transport/mod.rs";
  const adminNativeRootPath =
    "crates/rustok-search/admin/src/transport/native_server_adapter.rs";
  const adminNativeMappingPath =
    "crates/rustok-search/admin/src/transport/native_server_adapter/mapping.rs";
  const adminShellPath = "apps/admin/src/widgets/app_shell/native_server_adapter.rs";
  const compatibilityPath =
    "crates/rustok-search/storefront/src/transport/navigation.rs";

  write(
    root,
    enginePath,
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
    graphqlPath,
    localGraphqlPolicy
      ? "fn derive_search_result_url(_: &SearchResultItem) -> Option<String> { None }"
      : "let url = crate::canonical_search_result_url(&value);",
  );
  write(
    root,
    storefrontNativePath,
    missingStorefrontNativeDelegation
      ? "let url = None;"
      : "let url = rustok_search::canonical_search_result_url(&value);",
  );
  write(
    root,
    storefrontFacadePath,
    transportFallbackRegression
      ? "mod navigation; navigation::enrich_search_result_urls(&mut payload); blog_result_url"
      : "execute_selected_transport(...).await",
  );
  write(
    root,
    adminNativeRootPath,
    'include!("native_server_adapter/mapping.rs");',
  );
  write(
    root,
    adminNativeMappingPath,
    missingAdminNativeDelegation
      ? "fn derive_search_result_url(_: &SearchResultItem) -> Option<String> { None }"
      : "let url = rustok_search::canonical_search_result_url(&item);",
  );
  write(
    root,
    adminShellPath,
    missingAdminShellDelegation
      ? "fn derive_admin_search_result_url(_: &SearchResultItem) -> Option<String> { None }"
      : "let url = rustok_search::canonical_search_result_url(&item);",
  );
  if (transportFallbackRegression) {
    write(root, compatibilityPath, "fn blog_result_url() {} enrich_search_result_urls");
  }

  const productionContract = {
    normalized_result: enginePath,
    public_export: "crates/rustok-search/src/lib.rs",
    graphql_projection: graphqlPath,
    storefront_native_projection: storefrontNativePath,
    storefront_transport_facade: storefrontFacadePath,
    admin_native_root: adminNativeRootPath,
    admin_native_mapping: adminNativeMappingPath,
    admin_shell_projection: adminShellPath,
  };
  if (staleEvidenceFallback) {
    productionContract.compatibility_fallback = compatibilityPath;
  }
  write(
    root,
    "crates/rustok-search/contracts/evidence/search-canonical-url-contract.json",
    JSON.stringify({
      schema_version: 1,
      module: "search",
      surface: "canonical_result_url",
      status: "source_verified_no_compile",
      compile_policy: "not_run_by_request",
      production_contract: productionContract,
      cases: [
        "blog_canonical_route",
        "blog_fail_closed",
        "product_and_content_routes",
        "content_kind_injection",
        "graphql_owner_projection",
        "storefront_native_owner_projection",
        "admin_native_owner_projection",
        "admin_shell_owner_projection",
        "no_transport_fallback",
      ].map((name) => ({ name })),
    }),
  );
  write(
    root,
    "crates/rustok-search/docs/implementation-plan.md",
    "search-canonical-url-contract.json canonical_search_result_url single owner policy no transport fallback",
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

function expectRejected(options, pattern) {
  const root = fixture(options);
  try {
    const result = run(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("accepts one Search-owned canonical URL policy", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("rejects a transport-local GraphQL switch", () => {
  expectRejected(
    { localGraphqlPolicy: true },
    /missing crate::canonical_search_result_url|forbidden fn derive_search_result_url/,
  );
});

test("rejects missing Blog source ownership", () => {
  expectRejected(
    { missingBlogSourceCheck: true },
    /missing value.source_module == BLOG_SOURCE_MODULE/,
  );
});

test("rejects missing storefront native delegation", () => {
  expectRejected(
    { missingStorefrontNativeDelegation: true },
    /missing rustok_search::canonical_search_result_url\(&value\)/,
  );
});

test("rejects admin package URL duplication", () => {
  expectRejected(
    { missingAdminNativeDelegation: true },
    /missing rustok_search::canonical_search_result_url\(&item\)|forbidden fn derive_search_result_url/,
  );
});

test("rejects admin shell URL duplication", () => {
  expectRejected(
    { missingAdminShellDelegation: true },
    /missing rustok_search::canonical_search_result_url\(&item\)|forbidden fn derive_admin_search_result_url/,
  );
});

test("rejects post-transport compatibility enrichment", () => {
  expectRejected(
    { transportFallbackRegression: true },
    /compatibility implementation must be deleted|forbidden mod navigation|forbidden enrich_search_result_urls/,
  );
});

test("rejects stale fallback evidence", () => {
  expectRejected(
    { staleEvidenceFallback: true },
    /compatibility_fallback must be removed/,
  );
});
