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
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

const controlsPath = "crates/rustok-product/storefront/src/catalog_controls.rs";
const ownerTypesPath = "crates/rustok-product/src/services/catalog/types.rs";
const ownerQueriesPath = "crates/rustok-product/src/services/catalog/queries.rs";
const nativePath = "crates/rustok-product/storefront/src/transport/catalog_list_native.rs";
const graphqlAdapterPath = "crates/rustok-product/storefront/src/transport/graphql_adapter.rs";
const graphqlResolverPath = "crates/rustok-commerce/src/graphql/product_catalog.rs";
const graphqlRootPath = "crates/rustok-commerce/src/graphql/mod.rs";
const uiPath = "crates/rustok-product/storefront/src/ui/leptos.rs";
const planPath = "crates/rustok-product/docs/implementation-plan.md";

const controls = read(controlsPath);
const ownerTypes = read(ownerTypesPath);
const ownerQueries = read(ownerQueriesPath);
const native = read(nativePath);
const graphqlAdapter = read(graphqlAdapterPath);
const graphqlResolver = read(graphqlResolverPath);
const graphqlRoot = read(graphqlRootPath);
const ui = read(uiPath);
const plan = read(planPath);

for (const marker of [
  "pub category_id: Option<String>",
  "pub sort_by: Option<String>",
  "pub sort_direction: Option<String>",
  "normalize_category_id",
  "normalize_sort_by",
  "normalize_sort_direction",
]) {
  requireText(controls, marker, `${controlsPath}: missing typed control marker ${marker}`);
}

for (const marker of [
  "pub category_id: Option<Uuid>",
  "StorefrontProductSortBy",
  "StorefrontProductSortDirection",
  "try_from_transport",
]) {
  requireText(ownerTypes, marker, `${ownerTypesPath}: missing owner query marker ${marker}`);
}

for (const marker of [
  "PrimaryCategoryId.eq(category_id)",
  "StorefrontProductSortBy::PublishedAt",
  "StorefrontProductSortBy::CreatedAt",
  "StorefrontProductSortDirection::Asc",
  "StorefrontProductSortDirection::Desc",
]) {
  requireText(ownerQueries, marker, `${ownerQueriesPath}: missing owner execution marker ${marker}`);
}

for (const marker of [
  "controls.category_id",
  "controls.sort_by",
  "controls.sort_direction",
  "StorefrontProductListQuery::try_from_transport",
]) {
  requireText(native, marker, `${nativePath}: missing native transport marker ${marker}`);
}

for (const marker of [
  "storefrontProductCatalog",
  "category_id: controls.category_id",
  "sort_by: controls.sort_by",
  "sort_direction: controls.sort_direction",
]) {
  requireText(graphqlAdapter, marker, `${graphqlAdapterPath}: missing GraphQL adapter marker ${marker}`);
}

for (const marker of [
  "pub struct StorefrontProductCatalogFilter",
  "pub category_id: Option<Uuid>",
  "StorefrontProductListQuery::try_new",
  ".list_published_products_with_query(",
]) {
  requireText(graphqlResolver, marker, `${graphqlResolverPath}: missing GraphQL resolver marker ${marker}`);
}
requireText(
  graphqlRoot,
  "product_catalog::ProductCatalogQuery",
  `${graphqlRootPath}: Product catalog resolver must be merged into the Commerce query root`,
);

for (const marker of [
  'read_route_query_value(&route_context, "category_id")',
  'read_route_query_value(&route_context, "sort_by")',
  'read_route_query_value(&route_context, "sort_direction")',
  'name="category_id"',
  'name="sort_by"',
  'name="sort_direction"',
  "fetch_catalog_search_options",
]) {
  requireText(ui, marker, `${uiPath}: missing storefront UI marker ${marker}`);
}

requireText(
  plan,
  "Connect storefront category and deterministic date sorting",
  `${planPath}: completed category/sort slice must be recorded`,
);
requireText(
  plan,
  "verify-product-storefront-category-sort.mjs",
  `${planPath}: category/sort verifier must be listed`,
);

if (failures.length > 0) {
  console.error("product storefront category/sort verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("product storefront category/sort verification passed");
