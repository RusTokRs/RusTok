#!/usr/bin/env node
/**
 * Source gate for the Next storefront facet panel.
 *
 * The panel is the last link of one chain: the Product owner counts the buckets, Commerce GraphQL
 * exposes them as `storefrontProductCatalogFacets`, the Next package asks for them, and the catalog
 * page renders them as URL-driven drill-down links. This verifier source-locks every link of that
 * chain so a refactor cannot silently drop one of them, and it pins the two contracts the panel
 * must not invent locally:
 *
 *   - `attribute_filters` stays the single `;`-joined `code=value` route value the Rust storefront
 *     and the owner already speak (`rustok-product-storefront`, `rustok-product`);
 *   - the facet request never paginates: the owner counts the whole filtered catalog.
 */

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!existsSync(absolutePath)) {
    failures.push(`${relativePath}: required storefront facet file is missing`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function requireAll(source, markers, description) {
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${description}: missing ${marker}`);
  }
}

function reject(source, markers, description) {
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${description}: forbidden ${marker}`);
  }
}

const packageRoot = "apps/next-frontend/packages/rustok-product/src";
const facets = read(`${packageRoot}/catalog/facets.ts`);
const apiProducts = read(`${packageRoot}/api/products.ts`);
const apiTypes = read(`${packageRoot}/api/types.ts`);
const filters = read(`${packageRoot}/components/product-filters.tsx`);
const grid = read(`${packageRoot}/components/product-grid.tsx`);
const index = read(`${packageRoot}/index.tsx`);
const page = read("apps/next-frontend/src/app/[locale]/products/page.tsx");
const rustControls = read(
  "crates/modules/rustok-product/storefront/src/catalog_controls.rs",
);
const graphqlRoot = read("crates/modules/rustok-commerce/src/graphql/product_catalog.rs");
const sharedGrid = read("packages/rustok-ui-grid/src/facet.ts");
const sharedPanel = read("packages/rustok-ui-grid/src/panel.ts");
const sharedUrl = read("packages/rustok-ui-grid/src/url.ts");

requireAll(
  facets,
  [
    "export function parseAttributeFilter",
    "export function parseAttributeFilters",
    "export function serializeAttributeFilters",
    "export function isAttributeFilterSelected",
    "export function hasAttributeFilterForCode",
    "export function toggleAttributeFilter",
    "export function clearAttributeFilterCode",
    "export function buildCatalogFacetCodes",
    "export function buildCatalogFacetToggleQuery",
    "export function buildCatalogFacetClearCodeQuery",
    "export function buildCatalogFacetClearQuery",
    "export function buildCatalogFacetFiltersView",
    "export function buildCatalogFacetLabels",
  ],
  "storefront facet core",
);
// The storefront keeps no private copy of the table vocabulary: the generic half is imported from
// the host table toolkit (the TypeScript twin of `crates/ui/rustok-grid`) and re-exported.
requireAll(
  facets,
  [
    'from "@rustok/ui-grid"',
    "buildFacetPanel(",
    "facetFromBuckets(",
    "catalogFacetToSource(",
  ],
  "storefront facet shared core",
);
requireAll(
  sharedGrid,
  [
    "SELECTION_LIST_SEPARATOR",
    "selected.join(SELECTION_LIST_SEPARATOR)",
    "split(SELECTION_LIST_SEPARATOR)",
    "MAX_GRID_FACETS",
  ],
  "shared ui-grid selection contract",
);
requireAll(
  sharedUrl,
  ["URLSearchParams", "export function applyQueryPairs", "export function queryParam"],
  "shared ui-grid route contract",
);
requireAll(
  sharedPanel,
  ["facet.isTruncated", "bucket.count", "countLabel(", "marker:"],
  "shared ui-grid panel contract",
);
reject(
  facets,
  ['from "react"', 'from "next/', "useState"],
  "storefront facet core must stay framework-free",
);
reject(
  facets,
  ["URLSearchParams", "SELECTION_LIST_SEPARATOR"],
  "storefront facet core must delegate the vocabulary instead of re-implementing it",
);

requireAll(
  apiProducts,
  [
    "const STOREFRONT_CATALOG_FACETS_QUERY",
    "storefrontProductCatalogFacets(",
    "facetCodes",
    "export async function fetchStorefrontCatalogFacets",
    "toCatalogFilterVariables(filter, false)",
  ],
  "storefront facet GraphQL client",
);
requireAll(
  apiProducts,
  ["toCatalogFilterVariables(filter, true)"],
  "storefront catalog list keeps its paginated filter variables",
);
requireAll(
  apiTypes,
  [
    "export type ProductCatalogFacetValue",
    "export type ProductCatalogFacet",
    "isEnumerable: boolean",
    "isTruncated: boolean",
    "totalProducts: number",
  ],
  "storefront facet types",
);

requireAll(
  filters,
  [
    "buildCatalogFacetFiltersView(",
    "buildCatalogFacetLabels(locale)",
    "facetPanel.facets.map",
    "facetPanel.clearHref",
    "facet.unboundedHint",
    "facet.truncatedHint",
    "href={bucket.href}",
    "aria-pressed={bucket.selected}",
    "bucket.countLabel",
    "currentAttributeFilters.length > 0",
  ],
  "storefront facet panel UI",
);
requireAll(
  grid,
  [
    "currentAttributeFilters = []",
    "facets = []",
    "currencyCode",
    "applyQueryPairs(",
    "serializeAttributeFilters(currentAttributeFilters)",
    '["attribute_filters", serializedFilters || null]',
  ],
  "storefront facet grid plumbing",
);

requireAll(
  page,
  [
    "parseAttributeFilters(",
    "query.attribute_filters",
    "fetchStorefrontCatalogFacets(",
    "buildCatalogFacetCodes(searchOptions)",
    "currentAttributeFilters={attributeFilters}",
    "facets={facets}",
    "currencyCode={currencyCode}",
  ],
  "storefront catalog page wiring",
);

requireAll(
  index,
  [
    "buildCatalogFacetFiltersView",
    "buildCatalogFacetCodes",
    "parseAttributeFilters",
    "fetchStorefrontCatalogFacets",
    "ProductCatalogFacet",
  ],
  "storefront package exports",
);

// The panel speaks the vocabulary the Rust storefront and the owner already own.
requireAll(
  rustControls,
  ["filters.join(\";\")", "parse_attribute_filter"],
  "rust storefront attribute-filter vocabulary",
);
requireAll(
  graphqlRoot,
  ["storefront_product_catalog_facets", "StorefrontCatalogFacet"],
  "owner GraphQL facet root",
);

if (failures.length > 0) {
  console.error("product storefront next facet verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("product storefront next facet verification passed");
