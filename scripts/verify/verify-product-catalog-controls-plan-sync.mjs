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

function complete(relativePath, markers) {
  const source = read(relativePath);
  return markers.every((marker) => source.includes(marker));
}

function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

function forbidText(source, marker, message) {
  if (source.includes(marker)) failures.push(message);
}

const planPath = "crates/rustok-product/docs/implementation-plan.md";
const registryPath = "docs/modules/implementation-plans-registry.md";
const plan = read(planPath);
const registry = read(registryPath);

const storefrontSliceComplete = [
  complete("crates/rustok-product/storefront/src/catalog_controls.rs", [
    "pub category_id: Option<String>",
    "pub sort_by: Option<String>",
    "pub sort_direction: Option<String>",
  ]),
  complete("crates/rustok-product/storefront/src/ui/leptos.rs", [
    'name="category_id"',
    'name="sort_by"',
    'name="sort_direction"',
  ]),
  complete("crates/rustok-product/storefront/src/transport/catalog_list_native.rs", [
    "StorefrontProductListQuery::try_from_transport",
  ]),
  complete("crates/rustok-product/storefront/src/transport/graphql_adapter.rs", [
    "category_id: controls.category_id",
    "sort_by: controls.sort_by",
    "sort_direction: controls.sort_direction",
  ]),
  complete("crates/rustok-product/src/services/catalog/queries.rs", [
    "PrimaryCategoryId.eq(category_id)",
    "StorefrontProductSortBy::PublishedAt",
    "StorefrontProductSortBy::CreatedAt",
  ]),
].every(Boolean);

const adminSliceComplete = [
  complete("crates/rustok-product/admin/src/catalog_controls.rs", [
    "ProductAdminListInput",
    "pub category_id: Option<String>",
    "pub sort_by: Option<String>",
    "pub sort_direction: Option<String>",
  ]),
  complete("crates/rustok-product/admin/src/ui/catalog_admin.rs", [
    'name="category_id"',
    'name="sort_by"',
    'name="sort_direction"',
    "provide_context(catalog_controls)",
  ]),
  complete("crates/rustok-product/admin/src/catalog_transport.rs", [
    "use_context::<ProductAdminListInput>()",
    "admin_catalog_native::fetch_products",
    "admin_catalog_graphql::fetch_products",
  ]),
  complete("crates/rustok-product/admin/src/transport/admin_catalog_native.rs", [
    "AdminProductListQuery::try_from_transport",
    "list_admin_products_with_query",
  ]),
  complete("crates/rustok-product/admin/src/transport/admin_catalog_graphql.rs", [
    "AdminProductCatalogFilter",
    "categoryId",
    "sortBy",
    "sortDirection",
  ]),
  complete("crates/rustok-product/src/services/catalog/admin_queries.rs", [
    "Status.eq(status)",
    "PrimaryCategoryId.eq(category_id)",
    "order_by_asc",
    "order_by_desc",
  ]),
].every(Boolean);

const attributeFiltersComplete = [
  "crates/rustok-product/storefront/src/catalog_controls.rs",
  "crates/rustok-product/storefront/src/ui/leptos.rs",
  "crates/rustok-product/storefront/src/transport/catalog_list_native.rs",
  "crates/rustok-product/storefront/src/transport/graphql_adapter.rs",
  "crates/rustok-product/admin/src/catalog_controls.rs",
  "crates/rustok-product/admin/src/ui/catalog_admin.rs",
  "crates/rustok-product/admin/src/transport/admin_catalog_native.rs",
  "crates/rustok-product/admin/src/transport/admin_catalog_graphql.rs",
  "crates/rustok-product/src/services/catalog/types.rs",
].every((sourcePath) => complete(sourcePath, ["attribute_filters"]));

const umbrellaPending = "- [ ] Connect storefront/admin UI controls to optional catalog filters/sorts.";
const umbrellaComplete = "- [x] Connect storefront/admin UI controls to optional catalog filters/sorts.";
if (plan.includes(umbrellaPending) === plan.includes(umbrellaComplete)) {
  failures.push(`${planPath}: umbrella catalog-controls task must have exactly one checkbox state`);
} else if (attributeFiltersComplete && plan.includes(umbrellaPending)) {
  failures.push(`${planPath}: umbrella task is pending after attribute_filters became source-complete`);
} else if (!attributeFiltersComplete && plan.includes(umbrellaComplete)) {
  failures.push(`${planPath}: umbrella task must remain pending until attribute_filters reach both surfaces`);
}

const storefrontMarker = "- [x] Connect storefront category and deterministic date sorting through typed UI state, native/GraphQL transports, and Product-owned server-side execution.";
const adminMarker = "- [x] Connect admin search/status/category and deterministic date sorting through typed UI state, native/GraphQL transports, and Product-owned server-side execution.";
if (storefrontSliceComplete) requireText(plan, storefrontMarker, `${planPath}: completed storefront category/sort slice is not recorded`);
if (adminSliceComplete) requireText(plan, adminMarker, `${planPath}: completed admin category/sort slice is not recorded`);
if (!storefrontSliceComplete && plan.includes(storefrontMarker)) failures.push(`${planPath}: storefront category/sort marker is complete without source parity`);
if (!adminSliceComplete && plan.includes(adminMarker)) failures.push(`${planPath}: admin category/sort marker is complete without source parity`);

forbidText(
  plan,
  "optional catalog filters/sorts, detached-value marker contract",
  `${planPath}: catalog controls must not be described as source-locked before end-to-end execution exists`,
);
for (const marker of [
  "Recheck on 2026-07-29",
  "typed `attribute_filters`",
  "node scripts/verify/verify-product-admin-category-sort.mjs",
  "node scripts/verify/verify-product-admin-category-sort.test.mjs",
  "node scripts/verify/verify-product-catalog-controls-plan-sync.mjs",
  "node scripts/verify/verify-product-catalog-controls-plan-sync.test.mjs",
]) {
  requireText(plan, marker, `${planPath}: missing catalog controls plan marker ${marker}`);
}

const productRegistryRow = registry
  .split("\n")
  .find((line) => line.startsWith("| `product` |"));
if (!productRegistryRow) {
  failures.push(`${registryPath}: product live-plan row is missing`);
} else if (!/attribute|catalog filter|filters\/sorts/i.test(productRegistryRow)) {
  failures.push(`${registryPath}: product nearest priority must retain the open attribute/catalog filter slice`);
}

if (failures.length > 0) {
  console.error("product catalog controls plan synchronization verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("product catalog controls plan synchronization verification passed");
