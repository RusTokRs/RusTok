#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-product-catalog-controls-plan-sync.mjs");

function write(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

const storefrontMarker = "- [x] Connect storefront category and deterministic date sorting through typed UI state, native/GraphQL transports, and Product-owned server-side execution.";
const adminMarker = "- [x] Connect admin search/status/category and deterministic date sorting through typed UI state, native/GraphQL transports, and Product-owned server-side execution.";

function plan({ umbrellaComplete = false, includeAdminMarker = true } = {}) {
  return `
# Product plan
Recheck on 2026-07-29. The task stays open for typed \`attribute_filters\`.
## Verification
- [${umbrellaComplete ? "x" : " "}] Connect storefront/admin UI controls to optional catalog filters/sorts.
${storefrontMarker}
${includeAdminMarker ? adminMarker : ""}
- node scripts/verify/verify-product-admin-category-sort.mjs
- node scripts/verify/verify-product-admin-category-sort.test.mjs
- node scripts/verify/verify-product-catalog-controls-plan-sync.mjs
- node scripts/verify/verify-product-catalog-controls-plan-sync.test.mjs
`;
}

function fixture({ attributeComplete = false, umbrellaComplete = false, includeAdminMarker = true, omitAdminSource = false } = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-catalog-plan-sync-"));
  const attribute = attributeComplete ? " attribute_filters " : "";
  write(root, "crates/rustok-product/storefront/src/catalog_controls.rs", `pub category_id: Option<String> pub sort_by: Option<String> pub sort_direction: Option<String>${attribute}`);
  write(root, "crates/rustok-product/storefront/src/ui/leptos.rs", `name="category_id" name="sort_by" name="sort_direction"${attribute}`);
  write(root, "crates/rustok-product/storefront/src/transport/catalog_list_native.rs", `StorefrontProductListQuery::try_from_transport${attribute}`);
  write(root, "crates/rustok-product/storefront/src/transport/graphql_adapter.rs", `category_id: controls.category_id sort_by: controls.sort_by sort_direction: controls.sort_direction${attribute}`);
  write(root, "crates/rustok-product/src/services/catalog/queries.rs", `PrimaryCategoryId.eq(category_id) StorefrontProductSortBy::PublishedAt StorefrontProductSortBy::CreatedAt`);

  write(root, "crates/rustok-product/admin/src/catalog_controls.rs", `ProductAdminListInput pub category_id: Option<String> pub sort_by: Option<String> pub sort_direction: Option<String>${attribute}`);
  write(root, "crates/rustok-product/admin/src/ui/leptos.rs", `name="category_id" name="sort_by" name="sort_direction"${attribute}`);
  write(root, "crates/rustok-product/admin/src/transport.rs", `admin_catalog_native::fetch_products admin_catalog_graphql::fetch_products`);
  write(root, "crates/rustok-product/admin/src/transport/admin_catalog_native.rs", `AdminProductListQuery::try_from_transport list_admin_products_with_query${attribute}`);
  write(root, "crates/rustok-product/admin/src/transport/admin_catalog_graphql.rs", `AdminProductCatalogFilter categoryId sortBy sortDirection${attribute}`);
  write(root, "crates/rustok-product/src/services/catalog/admin_queries.rs", omitAdminSource
    ? `PrimaryCategoryId.eq(category_id) order_by_asc order_by_desc`
    : `Status.eq(status) PrimaryCategoryId.eq(category_id) order_by_asc order_by_desc`);
  write(root, "crates/rustok-product/src/services/catalog/types.rs", attribute);
  write(root, "crates/rustok-product/docs/implementation-plan.md", plan({ umbrellaComplete, includeAdminMarker }));
  write(root, "docs/modules/implementation-plans-registry.md", `| product |\n| \`product\` | plan | in_progress | Complete attribute filters before provider promotion. |`);
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
    assert.notEqual(result.status, 0, "expected plan-sync mutation to fail");
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("catalog plan sync accepts completed storefront/admin date controls with open attributes", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /product catalog controls plan synchronization verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("catalog plan sync rejects completed umbrella before attributes", () => {
  reject({ umbrellaComplete: true }, /must remain pending/);
});

test("catalog plan sync rejects pending umbrella after attributes complete", () => {
  reject({ attributeComplete: true }, /pending after attribute_filters/);
});

test("catalog plan sync rejects missing completed admin marker", () => {
  reject({ includeAdminMarker: false }, /completed admin category\/sort slice/);
});

test("catalog plan sync rejects admin marker without owner status source", () => {
  reject({ omitAdminSource: true }, /admin category\/sort marker is complete without source parity/);
});
