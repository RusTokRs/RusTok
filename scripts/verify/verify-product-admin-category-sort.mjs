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
    failures.push(`${relativePath}: required admin category/sort file is missing`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

const files = {
  controls: read("crates/rustok-product/admin/src/catalog_controls.rs"),
  ui: read("crates/rustok-product/admin/src/ui/catalog_admin.rs"),
  transport: read("crates/rustok-product/admin/src/catalog_transport.rs"),
  graphql: read("crates/rustok-product/admin/src/transport/admin_catalog_graphql.rs"),
  native: read("crates/rustok-product/admin/src/transport/admin_catalog_native.rs"),
  ownerTypes: read("crates/rustok-product/src/services/catalog/types.rs"),
  ownerQuery: read("crates/rustok-product/src/services/catalog/admin_queries.rs"),
  graphqlRoot: read("crates/rustok-commerce/src/graphql/product_catalog.rs"),
  plan: read("crates/rustok-product/docs/implementation-plan.md"),
};

for (const marker of ["ProductAdminListInput", "category_id", "sort_by", "sort_direction", "published_at", "created_at", "desc", "asc"]) {
  requireText(files.controls, marker, `admin controls must retain ${marker}`);
}
for (const marker of ['name="category_id"', 'name="sort_by"', 'name="sort_direction"', "fetch_catalog_search_options", "super::leptos::ProductAdmin"]) {
  requireText(files.ui, marker, `admin UI must retain ${marker}`);
}
for (const marker of ["build_product_admin_list_input", 'browser_query_value("category_id")', "admin_catalog_native::fetch_products", "admin_catalog_graphql::fetch_products"]) {
  requireText(files.transport, marker, `admin transport facade must retain ${marker}`);
}
for (const marker of ["adminProductCatalog", "AdminProductCatalogFilter", "categoryId", "sortBy", "sortDirection", "primaryCategoryId"]) {
  requireText(files.graphql, marker, `admin GraphQL adapter must retain ${marker}`);
}
for (const marker of ["product/admin/catalog-list", "AdminProductListQuery::try_from_transport", "list_admin_products_with_query", "PRODUCTS_LIST"]) {
  requireText(files.native, marker, `admin native adapter must retain ${marker}`);
}
for (const marker of ["AdminProductListQuery", "status must be `draft`, `active`, or `archived`", "category_id", "sort_by", "sort_direction"]) {
  requireText(files.ownerTypes, marker, `owner input must retain ${marker}`);
}
for (const marker of ["TenantId.eq(tenant_id)", "Status.eq(status)", "PrimaryCategoryId.eq(category_id)", "order_by_asc", "order_by_desc", "Id)"]) {
  requireText(files.ownerQuery, marker, `owner execution must retain ${marker}`);
}
for (const marker of ["async fn admin_product_catalog", "require_commerce_permission", "product_query_tenant", "list_admin_products_with_query"]) {
  requireText(files.graphqlRoot, marker, `admin GraphQL root must retain ${marker}`);
}
requireText(
  files.plan,
  "Connect admin search/status/category and deterministic date sorting",
  "implementation plan must retain the completed admin category/sort slice",
);
requireText(
  files.plan,
  "verify-product-admin-category-sort.mjs",
  "implementation plan must list the admin category/sort guard",
);

if (failures.length > 0) {
  console.error("product admin category/sort verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("product admin category/sort verification passed");
