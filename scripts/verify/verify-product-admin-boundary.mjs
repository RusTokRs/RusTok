#!/usr/bin/env node
// RusTok product admin FFA boundary guardrails.
// Checks the composed catalog-controls wrapper together with the preserved editor.

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function readRepo(relativePath) {
  if (!existsSync(repoPath(relativePath))) {
    failures.push(`${relativePath}: required product admin boundary file is missing`);
    return "";
  }
  return readFileSync(repoPath(relativePath), "utf8");
}

function requireText(source, marker, message) {
  const found = typeof marker === "string" ? source.includes(marker) : marker.test(source);
  if (!found) failures.push(message);
}

function forbidText(source, marker, message) {
  const found = typeof marker === "string" ? source.includes(marker) : marker.test(source);
  if (found) failures.push(message);
}

const paths = {
  lib: "crates/rustok-product/admin/src/lib.rs",
  controls: "crates/rustok-product/admin/src/catalog_controls.rs",
  core: "crates/rustok-product/admin/src/core.rs",
  ui: "crates/rustok-product/admin/src/ui/leptos.rs",
  legacyUi: "crates/rustok-product/admin/src/ui/legacy_leptos.rs",
  transport: "crates/rustok-product/admin/src/transport.rs",
  legacyTransport: "crates/rustok-product/admin/src/transport/legacy.rs",
  graphql: "crates/rustok-product/admin/src/transport/admin_catalog_graphql.rs",
  native: "crates/rustok-product/admin/src/transport/admin_catalog_native.rs",
  ownerTypes: "crates/rustok-product/src/services/catalog/types.rs",
  ownerQuery: "crates/rustok-product/src/services/catalog/admin_queries.rs",
  graphqlRoot: "crates/rustok-commerce/src/graphql/product_catalog.rs",
  plan: "crates/rustok-product/docs/implementation-plan.md",
  registry: "docs/modules/registry.md",
  package: "package.json",
};

const source = Object.fromEntries(
  Object.entries(paths).map(([key, relativePath]) => [key, readRepo(relativePath)]),
);

for (const marker of ["mod catalog_controls;", "mod core;", "mod transport;", "mod ui;", "pub use ui::leptos::ProductAdmin;"]) {
  requireText(source.lib, marker, `${paths.lib}: missing crate composition marker ${marker}`);
}

for (const marker of ["pub(crate) struct ProductAdminListInput", "pub category_id: Option<String>", "pub sort_by: Option<String>", "pub sort_direction: Option<String>", "build_product_admin_list_input", "build_product_admin_catalog_controls_labels"]) {
  requireText(source.controls, marker, `${paths.controls}: missing typed admin catalog control ${marker}`);
}
for (const marker of ["leptos::", "#[component]", "#[server", "LocalResource", "web_sys::"]) {
  forbidText(source.core, marker, `${paths.core}: neutral admin core must stay framework-free (${marker})`);
}
for (const marker of ["ProductAdminOpenProductViewModel", "ProductAdminProductsLoadViewModel", "ProductAttributeEditorState", "build_save_command", "product_admin_selected_product_query_state", "product_admin_products_load_view_from_result"]) {
  requireText(source.core, marker, `${paths.core}: preserved editor core marker is missing (${marker})`);
}

for (const marker of ["mod legacy;", "build_product_admin_catalog_controls_labels", 'read_route_query_value(&route_context, "category_id")', 'name="category_id"', 'name="sort_by"', 'name="sort_direction"', "transport::fetch_catalog_search_options", "<legacy::ProductAdmin />"]) {
  requireText(source.ui, marker, `${paths.ui}: missing composed admin UI marker ${marker}`);
}
for (const marker of ["TypedProductAttributeField", "build_save_command", "product_admin_products_load_view_from_result", "transport::fetch_products", "save_product_attribute_values"]) {
  requireText(source.legacyUi, marker, `${paths.legacyUi}: preserved editor marker is missing (${marker})`);
}
for (const marker of ["crate::api", "ProductService", "PricingService", "#[server"]) {
  forbidText(source.ui, marker, `${paths.ui}: catalog wrapper must not call raw services or endpoints (${marker})`);
}

for (const marker of ["mod admin_catalog_graphql;", "mod admin_catalog_native;", "pub(crate) use legacy::*;", "build_product_admin_list_input", 'browser_query_value("category_id")', 'browser_query_value("sort_by")', 'browser_query_value("sort_direction")', "admin_catalog_native::fetch_products", "admin_catalog_graphql::fetch_products"]) {
  requireText(source.transport, marker, `${paths.transport}: missing native-first typed catalog facade marker ${marker}`);
}
for (const marker of ["fetch_bootstrap", "fetch_product", "fetch_product_pricing", "fetch_catalog_categories", "fetch_effective_product_form", "save_product_attribute_values", "create_product", "update_product", "delete_product"]) {
  requireText(source.legacyTransport, marker, `${paths.legacyTransport}: preserved admin transport operation is missing (${marker})`);
}

for (const marker of ["ProductAdminCatalog", "adminProductCatalog", "AdminProductCatalogFilter", "categoryId", "sortBy", "sortDirection", "primaryCategoryId", "GraphqlRequest"] ) {
  requireText(source.graphql, marker, `${paths.graphql}: missing typed admin GraphQL mapping ${marker}`);
}
for (const marker of ['endpoint = "product/admin/catalog-list"', "HostRuntimeContext", "TransactionalEventBus", "PRODUCTS_LIST", "AdminProductListQuery::try_from_transport", ".list_admin_products_with_query(", "primary_category_id"]) {
  requireText(source.native, marker, `${paths.native}: missing Product-owned native admin list marker ${marker}`);
}
for (const marker of ["pub struct AdminProductListQuery", "pub status:", "pub category_id:", "sort_by:", "sort_direction:", "status must be `draft`, `active`, or `archived`"]) {
  requireText(source.ownerTypes, marker, `${paths.ownerTypes}: missing owner request validation marker ${marker}`);
}
for (const marker of ["pub async fn list_admin_products_with_query", "TenantId.eq(tenant_id)", "Status.eq(status)", "PrimaryCategoryId.eq(category_id)", "admin_product_title_search_condition", "order_by_asc", "order_by_desc", "shipping_profile_slug", "primary_category_id"]) {
  requireText(source.ownerQuery, marker, `${paths.ownerQuery}: missing owner-side admin list execution marker ${marker}`);
}
for (const marker of ["AdminProductCatalogFilter", "async fn admin_product_catalog", "require_commerce_permission", "product_query_tenant", "AdminProductListQuery::try_from_transport", ".list_admin_products_with_query("]) {
  requireText(source.graphqlRoot, marker, `${paths.graphqlRoot}: missing Product-backed admin GraphQL root marker ${marker}`);
}

requireText(source.plan, "verify-product-admin-boundary.mjs", `${paths.plan}: implementation plan must retain the admin boundary guard`);
requireText(source.registry, "verify-product-admin-boundary.mjs", `${paths.registry}: central registry must retain the admin boundary guard`);
requireText(source.package, "verify:product:admin-boundary", `${paths.package}: package scripts must expose admin boundary verification`);
requireText(source.package, "test:verify:product:admin-boundary", `${paths.package}: package scripts must expose admin boundary fixture tests`);

if (failures.length > 0) {
  console.error("product admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("product admin boundary verification passed");
