#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-product-admin-category-sort.mjs");

function write(root, relativePath, content) {
  const absolutePath = path.join(root, relativePath);
  mkdirSync(path.dirname(absolutePath), { recursive: true });
  writeFileSync(absolutePath, content);
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-admin-category-sort-"));
  write(root, "crates/rustok-product/admin/src/catalog_controls.rs", `ProductAdminListInput category_id sort_by sort_direction published_at created_at desc asc`);
  write(root, "crates/rustok-product/admin/src/ui/catalog_admin.rs", options.omitUiCategory
    ? `name="sort_by" name="sort_direction" fetch_catalog_search_options super::leptos::ProductAdmin`
    : `name="category_id" name="sort_by" name="sort_direction" fetch_catalog_search_options super::leptos::ProductAdmin`);
  write(root, "crates/rustok-product/admin/src/catalog_transport.rs", `build_product_admin_list_input browser_query_value("category_id") admin_catalog_native::fetch_products admin_catalog_graphql::fetch_products`);
  write(root, "crates/rustok-product/admin/src/transport/admin_catalog_graphql.rs", options.omitGraphqlSort
    ? `adminProductCatalog AdminProductCatalogFilter categoryId primaryCategoryId`
    : `adminProductCatalog AdminProductCatalogFilter categoryId sortBy sortDirection primaryCategoryId`);
  write(root, "crates/rustok-product/admin/src/transport/admin_catalog_native.rs", `product/admin/catalog-list AdminProductListQuery::try_from_transport list_admin_products_with_query PRODUCTS_LIST`);
  write(root, "crates/rustok-product/src/services/catalog/types.rs", `AdminProductListQuery status must be \`draft\`, \`active\`, or \`archived\` category_id sort_by sort_direction`);
  write(root, "crates/rustok-product/src/services/catalog/admin_queries.rs", options.omitOwnerCategory
    ? `TenantId.eq(tenant_id) Status.eq(status) order_by_asc order_by_desc Id)`
    : `TenantId.eq(tenant_id) Status.eq(status) PrimaryCategoryId.eq(category_id) order_by_asc order_by_desc Id)`);
  write(root, "crates/rustok-commerce/src/graphql/product_catalog.rs", `async fn admin_product_catalog require_commerce_permission product_query_tenant list_admin_products_with_query`);
  write(root, "crates/rustok-product/docs/implementation-plan.md", `Connect admin search/status/category and deterministic date sorting\nverify-product-admin-category-sort.mjs`);
  return root;
}

function run(root) {
  return spawnSync("node", [scriptPath], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

function reject(options, pattern) {
  const root = fixture(options);
  try {
    const result = run(root);
    assert.notEqual(result.status, 0, "expected mutation fixture to fail");
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("admin category/sort guard passes canonical fixture", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /product admin category\/sort verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("admin category/sort guard rejects missing UI category", () => {
  reject({ omitUiCategory: true }, /admin UI must retain/);
});

test("admin category/sort guard rejects missing GraphQL sort mapping", () => {
  reject({ omitGraphqlSort: true }, /admin GraphQL adapter must retain/);
});

test("admin category/sort guard rejects missing owner category execution", () => {
  reject({ omitOwnerCategory: true }, /owner execution must retain/);
});
