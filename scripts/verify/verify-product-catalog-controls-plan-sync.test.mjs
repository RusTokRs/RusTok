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
const attributeMarker = "- [x] Connect typed attribute_filters through storefront/admin UI state, native/GraphQL transports, filterable-definition validation, and Product-owned typed EAV execution.";

function plan({
  umbrellaComplete = true,
  includeAdminMarker = true,
  includeAttributeMarker = true,
  staleSourceLock = false,
  includeProvenance = true,
} = {}) {
  const provenance = includeProvenance
    ? "Recheck on 2026-07-29."
    : "The latest source recheck date is missing.";
  const sourceLock = staleSourceLock
    ? "The optional catalog filters/sorts, detached-value marker contract, and no-compile schema guardrail are source-locked."
    : "The complete `attribute_filters` through typed UI state contract is source-backed.";
  return `
# Product plan
${provenance} ${sourceLock}
## Verification
- [${umbrellaComplete ? "x" : " "}] Connect storefront/admin UI controls to optional catalog filters/sorts.
${storefrontMarker}
${includeAdminMarker ? adminMarker : ""}
${includeAttributeMarker ? attributeMarker : ""}
- node scripts/verify/verify-product-catalog-attribute-filters.mjs
- node scripts/verify/verify-product-catalog-attribute-filters.test.mjs
- node scripts/verify/verify-product-catalog-controls-plan-sync.mjs
- node scripts/verify/verify-product-catalog-controls-plan-sync.test.mjs
`;
}

function fixture({
  umbrellaComplete = true,
  includeAdminMarker = true,
  includeAttributeMarker = true,
  omitAdminSource = false,
  omitAttributeExecution = false,
  staleSourceLock = false,
  includeProvenance = true,
  providerPriority = true,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-catalog-plan-sync-"));

  write(root, "crates/rustok-product/storefront/src/catalog_controls.rs", `pub category_id: Option<String> pub sort_by: Option<String> pub sort_direction: Option<String> pub attribute_filters: Vec<String> serialize_attribute_filters`);
  write(root, "crates/rustok-product/storefront/src/ui/leptos.rs", `name="category_id" name="sort_by" name="sort_direction" read_route_query_value(&route_context, "attribute_filters") name="attribute_filters"`);
  write(root, "crates/rustok-product/storefront/src/transport/catalog_list_native.rs", `StorefrontProductListQuery::try_from_transport try_from_transport_with_attribute_filters attribute_filters: Vec<String>`);
  write(root, "crates/rustok-product/storefront/src/transport/graphql_adapter.rs", `category_id: controls.category_id sort_by: controls.sort_by sort_direction: controls.sort_direction attributeFilters attribute_filters: controls.attribute_filters`);
  write(root, "crates/rustok-product/src/services/catalog/queries.rs", `PrimaryCategoryId.eq(category_id) StorefrontProductSortBy::PublishedAt StorefrontProductSortBy::CreatedAt load_catalog_attribute_filter_conditions list_query.attribute_filters`);

  write(root, "crates/rustok-product/admin/src/catalog_controls.rs", `ProductAdminListInput pub category_id: Option<String> pub sort_by: Option<String> pub sort_direction: Option<String> pub attribute_filters: Vec<String> serialize_attribute_filters`);
  write(root, "crates/rustok-product/admin/src/ui/catalog_admin.rs", `name="category_id" name="sort_by" name="sort_direction" provide_context(catalog_controls) read_route_query_value(&route_context, "attribute_filters") name="attribute_filters"`);
  write(root, "crates/rustok-product/admin/src/catalog_transport.rs", `use_context::<ProductAdminListInput>() admin_catalog_native::fetch_products admin_catalog_graphql::fetch_products`);
  write(root, "crates/rustok-product/admin/src/transport/admin_catalog_native.rs", `AdminProductListQuery::try_from_transport try_from_transport_with_attribute_filters list_admin_products_with_query attribute_filters: Vec<String>`);
  write(root, "crates/rustok-product/admin/src/transport/admin_catalog_graphql.rs", `AdminProductCatalogFilter categoryId sortBy sortDirection attributeFilters attribute_filters: controls.attribute_filters`);
  write(root, "crates/rustok-product/src/services/catalog/admin_queries.rs", omitAdminSource
    ? `PrimaryCategoryId.eq(category_id) order_by_asc order_by_desc load_catalog_attribute_filter_conditions list_query.attribute_filters`
    : `Status.eq(status) PrimaryCategoryId.eq(category_id) order_by_asc order_by_desc load_catalog_attribute_filter_conditions list_query.attribute_filters`);
  write(root, "crates/rustok-product/src/services/catalog/types.rs", `pub struct ProductAttributeFilter attribute_filters: Vec<ProductAttributeFilter> MAX_ATTRIBUTE_FILTERS`);
  write(root, "crates/rustok-product/src/services/catalog/attribute_filters.rs", omitAttributeExecution
    ? `is_filterable = TRUE product_attribute_value_translations product_attribute_value_options`
    : `is_filterable = TRUE pav.detached_at IS NULL product_attribute_value_translations product_attribute_value_options`);
  write(root, "crates/rustok-product/docs/implementation-plan.md", plan({
    umbrellaComplete,
    includeAdminMarker,
    includeAttributeMarker,
    staleSourceLock,
    includeProvenance,
  }));
  const priority = providerPriority
    ? "Execute the catalog read provider and declared consumer fallback profiles before transport_verified."
    : "Complete typed attribute filters before provider promotion.";
  write(root, "docs/modules/implementation-plans-registry.md", `| product |\n| \`product\` | plan | boundary_ready | ${priority} |`);
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

test("catalog plan sync accepts fully closed catalog controls", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /product catalog controls plan synchronization verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("catalog plan sync rejects pending umbrella after full source parity", () => {
  reject({ umbrellaComplete: false }, /pending after attribute_filters/);
});

test("catalog plan sync rejects completed umbrella without typed execution", () => {
  reject({ omitAttributeExecution: true }, /must remain pending|attribute_filters marker is complete without typed source parity/);
});

test("catalog plan sync rejects missing completed attribute marker", () => {
  reject({ includeAttributeMarker: false }, /completed attribute_filters slice/);
});

test("catalog plan sync rejects missing completed admin marker", () => {
  reject({ includeAdminMarker: false }, /completed admin category\/sort slice/);
});

test("catalog plan sync rejects admin marker without owner status source", () => {
  reject({ omitAdminSource: true }, /admin category\/sort marker is complete without source parity/);
});

test("catalog plan sync rejects stale source-locked claim", () => {
  reject({ staleSourceLock: true }, /must not be described as source-locked/);
});

test("catalog plan sync rejects missing source recheck provenance", () => {
  reject({ includeProvenance: false }, /Recheck on 2026-07-29/);
});

test("catalog plan sync rejects stale central registry priority", () => {
  reject({ providerPriority: false }, /nearest priority/);
});
