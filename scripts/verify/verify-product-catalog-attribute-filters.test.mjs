#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-product-catalog-attribute-filters.mjs");

function write(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-attribute-filter-"));
  write(root, "crates/modules/rustok-product/src/services/catalog/types.rs", `pub struct ProductAttributeFilter attribute_filters: Vec<ProductAttributeFilter> code=value MAX_ATTRIBUTE_FILTERS try_new_with_attribute_filters try_from_transport_with_attribute_filters`);
  write(root, "crates/modules/rustok-product/src/services/catalog/attribute_filters.rs", options.omitDetached
    ? `is_filterable = TRUE scope IN ('product', 'both') product_attribute_value_translations value_integer value_decimal value_boolean value_date value_datetime product_attribute_value_options product_attribute_options cannot be used in attribute_filters`
    : `is_filterable = TRUE scope IN ('product', 'both') pav.detached_at IS NULL product_attribute_value_translations value_integer value_decimal value_boolean value_date value_datetime product_attribute_value_options product_attribute_options cannot be used in attribute_filters`);
  write(root, "crates/modules/rustok-product/src/services/catalog/queries.rs", `load_catalog_attribute_filter_conditions list_query.attribute_filters`);
  write(root, "crates/modules/rustok-product/src/services/catalog/admin_queries.rs", `load_catalog_attribute_filter_conditions list_query.attribute_filters`);
  write(root, "crates/modules/rustok-product/storefront/src/catalog_controls.rs", [
    `pub attribute_filters: Vec<String> serialize_attribute_filters`,
    `build_catalog_facet_labels`,
    `toggle_attribute_filter`,
    `is_attribute_filter_selected`,
  ].join(" "));
  write(root, "crates/modules/rustok-product/storefront/src/ui/leptos.rs", options.omitStorefrontUi
    ? ``
    : [
        `read_route_query_value(&route_context, "attribute_filters")`,
        `name="attribute_filters"`,
        options.omitFacetPanelUi ? `` : `<CatalogFacetFilters build_catalog_facet_filters_view_model`,
      ].join(" "));
  write(root, "crates/modules/rustok-product/storefront/src/core.rs", options.omitFacetView
    ? `catalog facade without facet view models`
    : [
        `build_catalog_facet_codes`,
        `build_catalog_facet_filters_view_model`,
        `build_catalog_facet_toggle_query`,
        `apply_ui_query_pairs`,
      ].join(" "));
  write(root, "crates/ui/rustok-ui-core/src/ui.rs", `pub fn apply_ui_query_pairs`);
  write(root, "crates/modules/rustok-product/storefront/src/transport/catalog_list_native.rs", `attribute_filters: Vec<String> try_from_transport_with_attribute_filters`);
  write(root, "crates/modules/rustok-product/storefront/src/transport/catalog_facets_native.rs", options.omitFacetNative
    ? `storefront_catalog_facets_native`
    : [
        `storefront_catalog_facets_native`,
        `storefront_catalog_facets`,
        `attribute_filters: Vec<String>`,
        `facet_codes: Vec<String>`,
      ].join(" "));
  write(root, "crates/modules/rustok-product/storefront/src/transport/graphql_adapter.rs", options.omitFacetGraphql
    ? `attributeFilters attribute_filters: controls.attribute_filters`
    : [
        `attributeFilters`,
        `attribute_filters: controls.attribute_filters`,
        `STOREFRONT_CATALOG_FACETS_QUERY`,
        `facetCodes`,
        `fetch_catalog_facets`,
      ].join(" "));
  write(root, "crates/modules/rustok-product/admin/src/catalog_controls.rs", `pub attribute_filters: Vec<String> serialize_attribute_filters`);
  write(root, "crates/modules/rustok-product/admin/src/ui/catalog_admin.rs", `read_route_query_value(&route_context, "attribute_filters") name="attribute_filters" provide_context(catalog_controls)`);
  write(root, "crates/modules/rustok-product/admin/src/transport/admin_catalog_native.rs", `attribute_filters: Vec<String> try_from_transport_with_attribute_filters`);
  write(root, "crates/modules/rustok-product/admin/src/transport/admin_catalog_graphql.rs", options.omitAdminGraphql ? `` : `attributeFilters attribute_filters: controls.attribute_filters`);
  write(root, "crates/modules/rustok-commerce/src/graphql/product_catalog.rs", options.omitFacetGraphql
    ? `pub attribute_filters: Vec<String> try_new_with_attribute_filters try_from_transport_with_attribute_filters`
    : `pub attribute_filters: Vec<String> try_new_with_attribute_filters try_from_transport_with_attribute_filters storefront_product_catalog_facets`);
  write(root, "crates/modules/rustok-product/docs/implementation-plan.md", [
    `- [x] Connect storefront/admin UI controls to optional catalog filters/sorts.`,
    `Connect typed attribute_filters through storefront/admin UI state`,
    options.omitFacetGraphql ? `` : `Render storefront facet buckets from the Product owner contract`,
    `verify-product-catalog-attribute-filters.mjs`,
  ].join("\n"));
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
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("attribute-filter guard passes canonical fixture", () => {
  const root = fixture();
  try {
    const result = run(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("attribute-filter guard rejects detached-value drift", () => {
  reject({ omitDetached: true }, /typed EAV execution/);
});

test("attribute-filter guard rejects missing storefront UI", () => {
  reject({ omitStorefrontUi: true }, /storefront UI/);
});

test("attribute-filter guard rejects missing admin GraphQL mapping", () => {
  reject({ omitAdminGraphql: true }, /admin GraphQL/);
});

test("attribute-filter guard rejects a missing storefront facet view model", () => {
  reject({ omitFacetView: true }, /storefront facet view model/);
});

test("attribute-filter guard rejects a missing storefront facet panel", () => {
  reject({ omitFacetPanelUi: true }, /storefront UI/);
});

test("attribute-filter guard rejects dropped storefront facet transports", () => {
  reject({ omitFacetNative: true }, /storefront facet native/);
  reject({ omitFacetGraphql: true }, /storefront GraphQL/);
});
