#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!existsSync(absolutePath)) {
    failures.push(`${relativePath}: required attribute-filter file is missing`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function requireAll(source, markers, description) {
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${description}: missing ${marker}`);
  }
}

const types = read("crates/rustok-product/src/services/catalog/types.rs");
const execution = read("crates/rustok-product/src/services/catalog/attribute_filters.rs");
const storefrontQuery = read("crates/rustok-product/src/services/catalog/queries.rs");
const adminQuery = read("crates/rustok-product/src/services/catalog/admin_queries.rs");
const storefrontControls = read("crates/rustok-product/storefront/src/catalog_controls.rs");
const storefrontUi = read("crates/rustok-product/storefront/src/ui/leptos.rs");
const storefrontNative = read("crates/rustok-product/storefront/src/transport/catalog_list_native.rs");
const storefrontGraphql = read("crates/rustok-product/storefront/src/transport/graphql_adapter.rs");
const adminControls = read("crates/rustok-product/admin/src/catalog_controls.rs");
const adminUi = read("crates/rustok-product/admin/src/ui/catalog_admin.rs");
const adminNative = read("crates/rustok-product/admin/src/transport/admin_catalog_native.rs");
const adminGraphql = read("crates/rustok-product/admin/src/transport/admin_catalog_graphql.rs");
const graphqlRoot = read("crates/rustok-commerce/src/graphql/product_catalog.rs");
const plan = read("crates/rustok-product/docs/implementation-plan.md");

requireAll(types, [
  "pub struct ProductAttributeFilter",
  "attribute_filters: Vec<ProductAttributeFilter>",
  "code=value",
  "MAX_ATTRIBUTE_FILTERS",
  "try_new_with_attribute_filters",
  "try_from_transport_with_attribute_filters",
], "owner input");
requireAll(execution, [
  "is_filterable = TRUE",
  "scope IN ('product', 'both')",
  "pav.detached_at IS NULL",
  "product_attribute_value_translations",
  "value_integer",
  "value_decimal",
  "value_boolean",
  "value_date",
  "value_datetime",
  "product_attribute_value_options",
  "product_attribute_options",
  "cannot be used in attribute_filters",
], "typed EAV execution");
requireAll(storefrontQuery, ["load_catalog_attribute_filter_conditions", "list_query.attribute_filters"], "storefront owner query");
requireAll(adminQuery, ["load_catalog_attribute_filter_conditions", "list_query.attribute_filters"], "admin owner query");
requireAll(storefrontControls, ["pub attribute_filters: Vec<String>", "serialize_attribute_filters"], "storefront controls");
requireAll(storefrontUi, ['read_route_query_value(&route_context, "attribute_filters")', 'name="attribute_filters"'], "storefront UI");
requireAll(storefrontNative, ["attribute_filters: Vec<String>", "try_from_transport_with_attribute_filters"], "storefront native");
requireAll(storefrontGraphql, ["attributeFilters", "attribute_filters: controls.attribute_filters"], "storefront GraphQL");
requireAll(adminControls, ["pub attribute_filters: Vec<String>", "serialize_attribute_filters"], "admin controls");
requireAll(adminUi, ['read_route_query_value(&route_context, "attribute_filters")', 'name="attribute_filters"', "provide_context(catalog_controls)"], "admin UI");
requireAll(adminNative, ["attribute_filters: Vec<String>", "try_from_transport_with_attribute_filters"], "admin native");
requireAll(adminGraphql, ["attributeFilters", "attribute_filters: controls.attribute_filters"], "admin GraphQL");
requireAll(graphqlRoot, ["pub attribute_filters: Vec<String>", "try_new_with_attribute_filters", "try_from_transport_with_attribute_filters"], "GraphQL roots");
requireAll(plan, [
  "- [x] Connect storefront/admin UI controls to optional catalog filters/sorts.",
  "Connect typed attribute_filters through storefront/admin UI state",
  "verify-product-catalog-attribute-filters.mjs",
], "implementation plan");

if (failures.length > 0) {
  console.error("product catalog attribute-filter verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("product catalog attribute-filter verification passed");
