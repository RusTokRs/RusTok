#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve(
  "scripts/verify/verify-product-storefront-category-sort.mjs",
);

function write(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function run(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-category-sort-"));
  try {
    write(
      root,
      "crates/rustok-product/storefront/src/catalog_controls.rs",
      `
pub struct CatalogListInput {
  pub category_id: Option<String>,
  pub sort_by: Option<String>,
  pub sort_direction: Option<String>,
}
fn normalize_category_id() {}
fn normalize_sort_by() {}
fn normalize_sort_direction() {}
`,
    );
    write(
      root,
      "crates/rustok-product/src/services/catalog/types.rs",
      `
pub category_id: Option<Uuid>
pub enum StorefrontProductSortBy {}
pub enum StorefrontProductSortDirection {}
pub fn try_from_transport() {}
`,
    );
    write(
      root,
      "crates/rustok-product/src/services/catalog/queries.rs",
      options.omitOwnerCategory
        ? `StorefrontProductSortBy::PublishedAt StorefrontProductSortBy::CreatedAt StorefrontProductSortDirection::Asc StorefrontProductSortDirection::Desc`
        : `PrimaryCategoryId.eq(category_id) StorefrontProductSortBy::PublishedAt StorefrontProductSortBy::CreatedAt StorefrontProductSortDirection::Asc StorefrontProductSortDirection::Desc`,
    );
    write(
      root,
      "crates/rustok-product/storefront/src/transport/catalog_list_native.rs",
      `
controls.category_id
controls.sort_by
controls.sort_direction
StorefrontProductListQuery::try_from_transport
`,
    );
    write(
      root,
      "crates/rustok-product/storefront/src/transport/graphql_adapter.rs",
      options.omitGraphqlSort
        ? `storefrontProductCatalog category_id: controls.category_id`
        : `storefrontProductCatalog category_id: controls.category_id sort_by: controls.sort_by sort_direction: controls.sort_direction`,
    );
    write(
      root,
      "crates/rustok-commerce/src/graphql/product_catalog.rs",
      `
pub struct StorefrontProductCatalogFilter
pub category_id: Option<Uuid>
StorefrontProductListQuery::try_new
.list_published_products_with_query(
`,
    );
    write(
      root,
      "crates/rustok-commerce/src/graphql/mod.rs",
      `product_catalog::ProductCatalogQuery`,
    );
    write(
      root,
      "crates/rustok-product/storefront/src/ui/leptos.rs",
      options.omitUiControls
        ? `fetch_catalog_search_options`
        : `
read_route_query_value(&route_context, "category_id")
read_route_query_value(&route_context, "sort_by")
read_route_query_value(&route_context, "sort_direction")
name="category_id"
name="sort_by"
name="sort_direction"
fetch_catalog_search_options
`,
    );
    write(
      root,
      "crates/rustok-product/docs/implementation-plan.md",
      options.omitPlanMarker
        ? `verify-product-storefront-category-sort.mjs`
        : `Connect storefront category and deterministic date sorting
verify-product-storefront-category-sort.mjs`,
    );

    return spawnSync("node", [scriptPath], {
      cwd: path.resolve("."),
      env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
      encoding: "utf8",
    });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("accepts the canonical storefront category and sort contract", () => {
  const result = run();
  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stdout, /verification passed/);
});

test("rejects a missing owner category predicate", () => {
  const result = run({ omitOwnerCategory: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /PrimaryCategoryId/);
});

test("rejects missing GraphQL sort propagation", () => {
  const result = run({ omitGraphqlSort: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /sort_by|sort_direction/);
});

test("rejects missing storefront query controls", () => {
  const result = run({ omitUiControls: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /category_id|sort_by|sort_direction/);
});

test("rejects missing plan completion marker", () => {
  const result = run({ omitPlanMarker: true });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /completed category\/sort slice/);
});
