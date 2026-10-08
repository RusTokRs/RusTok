#!/usr/bin/env node
/**
 * Source gate for admin catalog facets and the shared grid facet panel.
 *
 * The admin panel is the second consumer of the facet contract, and it has to stay a *consumer*:
 * the SQL belongs to the Product owner, the panel semantics belong to `rustok-grid`, and the admin
 * package only maps the owner's rows and renders links. This verifier source-locks that split so a
 * later refactor cannot move counting into the admin crate or re-invent the `code=value`
 * vocabulary inside a UI adapter.
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
    failures.push(`${relativePath}: required admin facet file is missing`);
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

const gridPanel = read("crates/ui/rustok-grid/src/facet_panel.rs");
const gridFacet = read("crates/ui/rustok-grid/src/facet.rs");
const gridLib = read("crates/ui/rustok-grid/src/lib.rs");
const ownerFacets = read("crates/modules/rustok-product/src/services/catalog/facets.rs");
const ownerPort = read("crates/modules/rustok-product/src/ports/catalog_read.rs");
const ownerPortTypes = read("crates/modules/rustok-product/src/ports/types.rs");
const graphqlRoot = read("crates/modules/rustok-commerce/src/graphql/product_catalog.rs");
const adminFacets = read("crates/modules/rustok-product/admin/src/facets.rs");
const adminModel = read("crates/modules/rustok-product/admin/src/model.rs");
const adminNative = read("crates/modules/rustok-product/admin/src/transport/admin_catalog_native.rs");
const adminGraphql = read(
  "crates/modules/rustok-product/admin/src/transport/admin_catalog_graphql.rs",
);
const adminTransport = read("crates/modules/rustok-product/admin/src/catalog_transport.rs");
const adminPanelUi = read("crates/modules/rustok-product/admin/src/ui/catalog_facets.rs");
const adminGrid = read("crates/modules/rustok-product/admin/src/ui/product_grid.rs");

// ── Shared panel: framework-free, selection in the `key=value` language ─────
requireAll(
  gridPanel,
  [
    "pub struct FacetPanelLabels",
    "pub struct FacetPanelValue",
    "pub struct FacetPanelFacet",
    "pub struct FacetPanel",
    "pub fn build(facets: &[GridFacet], selected: &[String], labels: &FacetPanelLabels)",
    "pub fn toggle(&self, key: &str, value: &str) -> Vec<String>",
    "pub fn clear_key(&self, key: &str) -> Vec<String>",
    "pub fn clear(&self) -> Vec<String>",
    "pub fn selection_after_toggle(selected: &[String], key: &str, value: &str) -> Vec<String>",
    "pub fn selection_after_clear_key(selected: &[String], key: &str) -> Vec<String>",
    "pub fn selection_after_clear(_selected: &[String]) -> Vec<String>",
    "pub fn is_selection_selected(selected: &[String], key: &str, value: &str) -> bool",
    "pub fn has_selection_for_key(selected: &[String], key: &str) -> bool",
    "pub fn count_label(template: &str, count: u64) -> String",
    "pub fn selection_entry(key: &str, value: &str) -> String",
    "pub fn split_selection(entry: &str) -> Option<(&str, &str)>",
    ".take(MAX_GRID_FACETS)",
  ],
  "grid facet panel",
);
reject(
  gridPanel,
  ["leptos", "wasm_bindgen", "web_sys", "use crate::filter::FilterOption {"],
  "grid facet panel must stay framework-free",
);
requireAll(
  gridLib,
  ["pub mod facet_panel;", "FacetPanel,", "selection_after_toggle,"],
  "grid facet panel exports",
);
requireAll(
  gridFacet,
  ["pub struct GridFacet", "pub const MAX_GRID_FACETS"],
  "grid facet contract",
);

// ── Owner: the SQL lives here, and it knows both scopes ────────────────────
requireAll(
  ownerFacets,
  [
    "pub async fn admin_catalog_facets(",
    "pub(super) enum CatalogFacetScope<'a>",
    "Storefront { public_channel_slug: Option<&'a str> }",
    "Admin { status: Option<&'a ProductStatus> }",
    "fn facet_status_literal(status: &ProductStatus) -> &'static str",
    "fn facet_scope_condition(",
    "pub(super) async fn load_catalog_facets(",
    "pub(super) struct CatalogFacetFilters<'a>",
    'ProductStatus::Draft => "draft"',
    'ProductStatus::Archived => "archived"',
    "CatalogFacetScope::Admin { status } => {",
  ],
  "owner facet scope",
);
reject(
  ownerFacets,
  ["products.status = {", "products.status = $"],
  "owner facet status comparison must stay an enum literal",
);

// ── Port: an optional admin capability that fails closed ───────────────────
requireAll(
  ownerPortTypes,
  [
    "pub struct AdminCatalogFacetsRequest",
    "pub query: AdminProductListQuery,",
    "pub facet_codes: Vec<String>,",
  ],
  "admin facet port request",
);
requireAll(
  ownerPort,
  [
    "async fn load_admin_catalog_facets(",
    'const LOAD_ADMIN_CATALOG_FACETS_OPERATION: &str = "load_admin_catalog_facets";',
    "product.admin_catalog_facets_unavailable",
    "crate::CatalogService::admin_catalog_facets(",
  ],
  "admin facet port",
);

// ── Commerce GraphQL: an admin root over the same bucket contract ──────────
requireAll(
  graphqlRoot,
  [
    "pub struct GqlAdminCatalogFacetValue",
    "pub struct GqlAdminCatalogFacet",
    "async fn admin_product_catalog_facets(",
    "load_admin_catalog_facets(",
    "rustok_product::AdminCatalogFacetsRequest {",
  ],
  "admin facet GraphQL root",
);

// ── Admin package: mapping plus rendering, never counting ─────────────────
requireAll(
  adminModel,
  [
    "pub struct AdminCatalogFacetValue",
    "pub struct AdminCatalogFacet",
    '#[serde(rename = "totalProducts")]',
    '#[serde(rename = "isEnumerable")]',
  ],
  "admin facet model",
);
requireAll(
  adminFacets,
  [
    "pub fn admin_catalog_facets_to_grid(",
    "FacetDomain::Dictionary {",
    "FacetDomain::Open",
    "FacetPanel::build(",
    "pub fn build_product_admin_facet_labels(",
    "pub fn build_product_admin_facet_codes(",
    "pub fn build_product_admin_facet_panel(",
    "selection_after_toggle(",
    "selection_after_clear_key(",
    "apply_ui_query_pairs(",
    "serialize_attribute_filters(",
    '"product.list.facetsLabel"',
  ],
  "admin facet mapping",
);
reject(
  [adminFacets, adminModel, adminTransport, adminGrid, adminPanelUi].join("\n"),
  ["SELECT ", "COUNT(DISTINCT", "products.status", "sea_orm"],
  "admin facet consumers must not count products themselves",
);
requireAll(
  adminNative,
  [
    'endpoint = "product/admin/catalog-facets"',
    "async fn product_admin_catalog_facets_native(",
    "admin_catalog_facets(",
    "pub(crate) async fn fetch_admin_catalog_facets(",
    "PRODUCTS_LIST",
  ],
  "admin facet native transport",
);
requireAll(
  adminGraphql,
  [
    "ADMIN_PRODUCT_CATALOG_FACETS_QUERY",
    "adminProductCatalogFacets(",
    "facetCodes",
    "pub(crate) async fn fetch_facets(",
  ],
  "admin facet GraphQL transport",
);
requireAll(
  adminTransport,
  [
    "pub(crate) async fn fetch_admin_catalog_facets(",
    "legacy::admin_catalog_native::fetch_admin_catalog_facets(",
    "legacy::admin_catalog_graphql::fetch_facets(",
    "-> Option<Vec<crate::model::AdminCatalogFacet>>",
  ],
  "admin facet transport facade",
);
requireAll(
  adminGrid,
  [
    "facets_resource",
    "build_product_admin_facet_codes(&options)",
    "catalog_transport::fetch_admin_catalog_facets(",
    "build_product_admin_facet_panel(",
    "<AdminCatalogFacetPanel panel />",
  ],
  "admin facet grid wiring",
);
requireAll(
  adminPanelUi,
  ["pub fn AdminCatalogFacetPanel(", "aria-pressed=", "data-facet=code"],
  "admin facet panel UI",
);

if (failures.length > 0) {
  console.error("product admin facet verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("product admin facet verification passed");
