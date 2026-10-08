#!/usr/bin/env node
/**
 * Source gate for the cross-layer facet chain (`PROD-FACET-CHAIN-001`).
 *
 * A facet is one number with four owners: the Product owner counts the buckets, the GraphQL layer
 * exposes them under two differently gated roots, each transport asks for the codes it can render,
 * and each panel maps the answer into the shared grid contract. Every seam in that chain can drift
 * silently — a field renamed in Rust and forgotten in a selection string, a storefront that starts
 * demanding the admin root, an owner that stops reporting truncation — and the numbers still look
 * plausible. This verifier locks the seams:
 *
 *   owner port (`load_*_catalog_facets`) → request shapes (codes in panel order, no pagination)
 *   → GraphQL contract (types, gates, deadline, error mapping) → both transports of both hosts
 *   (selection strings, serde renames, native endpoints) → both panels (codes derived from the same
 *   options they render, caps aligned with the owner, counters and truncation preserved).
 *
 * It also rejects the two ways the boundary can be crossed: a storefront or admin surface that
 * counts facets itself (SQL, attribute tables, `COUNT(`), and a surface that calls the other
 * scope's root.
 */

import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
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

/** Requires one whole line to match `pattern`; a substring check would accept `per_page: None,`. */
function requireLine(source, pattern, description, expected) {
  if (!pattern.test(source)) failures.push(`${description}: missing line ${expected}`);
}

function countOccurrences(source, marker) {
  return source.split(marker).length - 1;
}

/** Slice of `source` between two markers; both markers must exist, in that order. */
function sliceBetween(source, startMarker, endMarker, description) {
  const start = source.indexOf(startMarker);
  if (start < 0) {
    failures.push(`${description}: missing ${startMarker}`);
    return "";
  }
  const end = source.indexOf(endMarker, start + startMarker.length);
  if (end < 0) {
    failures.push(`${description}: missing ${endMarker} after ${startMarker}`);
    return "";
  }
  return source.slice(start, end);
}

/** Body of a Rust struct (or TS type object) declared by `name`, brace-balanced. */
function bodyAfter(source, declaration, description) {
  const start = source.indexOf(declaration);
  if (start < 0) {
    failures.push(`${description}: missing ${declaration}`);
    return "";
  }
  const open = source.indexOf("{", start);
  if (open < 0) {
    failures.push(`${description}: ${declaration} has no body`);
    return "";
  }
  let depth = 0;
  for (let index = open; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(open + 1, index);
    }
  }
  failures.push(`${description}: ${declaration} body is not closed`);
  return "";
}

/** First capture of `pattern` in `source`, or `undefined`. */
function capture(source, pattern) {
  const match = source.match(pattern);
  return match ? match[1] : undefined;
}

/** Every file under a directory, by extension, as `relativePath -> source`. */
function tree(directory, extensions) {
  const absolute = path.join(repoRoot, directory);
  const collected = [];
  if (!existsSync(absolute)) return collected;
  const walk = (current) => {
    for (const entry of readdirSync(current)) {
      if (entry === "node_modules" || entry === "target" || entry === ".next") continue;
      const entryPath = path.join(current, entry);
      if (statSync(entryPath).isDirectory()) {
        walk(entryPath);
        continue;
      }
      if (extensions.some((extension) => entry.endsWith(extension))) {
        collected.push({
          file: path.relative(repoRoot, entryPath),
          source: readFileSync(entryPath, "utf8"),
        });
      }
    }
  };
  walk(absolute);
  return collected;
}

const OWNER_PORT = "crates/modules/rustok-product/src/ports/catalog_read.rs";
const OWNER_PORT_TYPES = "crates/modules/rustok-product/src/ports/types.rs";
const OWNER_FACETS = "crates/modules/rustok-product/src/services/catalog/facets.rs";
const OWNER_FILTER_LIMITS = "crates/modules/rustok-product/src/services/catalog/types.rs";
const COMMERCE_FACETS = "crates/modules/rustok-commerce/src/graphql/product_catalog.rs";
const STOREFRONT_MODEL = "crates/modules/rustok-product/storefront/src/model.rs";
const STOREFRONT_CORE = "crates/modules/rustok-product/storefront/src/core.rs";
const STOREFRONT_UI = "crates/modules/rustok-product/storefront/src/ui/leptos.rs";
const STOREFRONT_TRANSPORT = "crates/modules/rustok-product/storefront/src/transport/mod.rs";
const STOREFRONT_GRAPHQL =
  "crates/modules/rustok-product/storefront/src/transport/graphql_adapter.rs";
const STOREFRONT_NATIVE =
  "crates/modules/rustok-product/storefront/src/transport/catalog_facets_native.rs";
const ADMIN_MODEL = "crates/modules/rustok-product/admin/src/model.rs";
const ADMIN_FACETS = "crates/modules/rustok-product/admin/src/facets.rs";
const ADMIN_UI = "crates/modules/rustok-product/admin/src/ui/product_grid.rs";
const ADMIN_TRANSPORT = "crates/modules/rustok-product/admin/src/catalog_transport.rs";
const ADMIN_GRAPHQL = "crates/modules/rustok-product/admin/src/transport/admin_catalog_graphql.rs";
const ADMIN_NATIVE = "crates/modules/rustok-product/admin/src/transport/admin_catalog_native.rs";
const GRID_FACET_RS = "crates/ui/rustok-grid/src/facet.rs";
const GRID_FACET_TS = "packages/rustok-ui-grid/src/facet.ts";
const NEXT_TYPES = "apps/next-frontend/packages/rustok-product/src/api/types.ts";
const NEXT_PRODUCTS = "apps/next-frontend/packages/rustok-product/src/api/products.ts";
const NEXT_FACETS = "apps/next-frontend/packages/rustok-product/src/catalog/facets.ts";
const NEXT_FILTERS = "apps/next-frontend/packages/rustok-product/src/components/product-filters.tsx";
const NEXT_PAGE = "apps/next-frontend/src/app/[locale]/products/page.tsx";

const ownerPort = read(OWNER_PORT);
const ownerPortTypes = read(OWNER_PORT_TYPES);
const ownerFacets = read(OWNER_FACETS);
const ownerFilterLimits = read(OWNER_FILTER_LIMITS);
const commerceFacets = read(COMMERCE_FACETS);
const storefrontModel = read(STOREFRONT_MODEL);
const storefrontCore = read(STOREFRONT_CORE);
const storefrontUi = read(STOREFRONT_UI);
const storefrontTransport = read(STOREFRONT_TRANSPORT);
const storefrontGraphql = read(STOREFRONT_GRAPHQL);
const storefrontNative = read(STOREFRONT_NATIVE);
const adminModel = read(ADMIN_MODEL);
const adminFacets = read(ADMIN_FACETS);
const adminUi = read(ADMIN_UI);
const adminTransport = read(ADMIN_TRANSPORT);
const adminGraphql = read(ADMIN_GRAPHQL);
const adminNative = read(ADMIN_NATIVE);
const gridFacetRs = read(GRID_FACET_RS);
const gridFacetTs = read(GRID_FACET_TS);
const nextTypes = read(NEXT_TYPES);
const nextProducts = read(NEXT_PRODUCTS);
const nextFacets = read(NEXT_FACETS);
const nextFilters = read(NEXT_FILTERS);
const nextPage = read(NEXT_PAGE);

// ── Owner: facets are a port capability with its own operation names ─────────────────────────────
requireAll(
  ownerPort,
  [
    'const LOAD_STOREFRONT_CATALOG_FACETS_OPERATION: &str = "load_storefront_catalog_facets";',
    'const LOAD_ADMIN_CATALOG_FACETS_OPERATION: &str = "load_admin_catalog_facets";',
    '"product.catalog_facets_unavailable",',
    '"product.admin_catalog_facets_unavailable",',
  ],
  "owner facet port contract",
);
requireAll(
  ownerPort,
  [
    "let owner_operation = LOAD_STOREFRONT_CATALOG_FACETS_OPERATION;",
    "let owner_operation = LOAD_ADMIN_CATALOG_FACETS_OPERATION;",
    "crate::CatalogService::storefront_catalog_facets(",
    "crate::CatalogService::admin_catalog_facets(",
    "request.facet_codes.as_slice(),",
    "request.fallback_locale.as_deref(),",
    "let locale = request.locale.as_deref().unwrap_or(context.locale.as_str());",
  ],
  "owner facet port implementations",
);
if (countOccurrences(ownerPort, "context.require_policy(PortCallPolicy::read())?;") < 2) {
  failures.push(
    `${OWNER_PORT}: both facet operations must go through the read policy before touching the owner`,
  );
}
// A facet projection is a read: an owner that stops counting for one scope must answer with the
// scope-specific unavailability code rather than an empty list the panel would render as zero.
const storefrontPortImpl = sliceBetween(
  ownerPort,
  "let owner_operation = LOAD_STOREFRONT_CATALOG_FACETS_OPERATION;",
  "let owner_operation = LOAD_ADMIN_CATALOG_FACETS_OPERATION;",
  "owner storefront facet port implementation",
);
const adminPortImpl = sliceBetween(
  ownerPort,
  "let owner_operation = LOAD_ADMIN_CATALOG_FACETS_OPERATION;",
  "async fn read_storefront_product_projection(",
  "owner admin facet port implementation",
);
requireAll(
  storefrontPortImpl,
  ["request.public_channel_slug.as_deref(),"],
  "owner storefront facet port implementation",
);
reject(
  adminPortImpl,
  ["public_channel_slug"],
  "owner admin facet port implementation must not be scoped to a public channel",
);
requireAll(adminPortImpl, ["request.facet_codes.as_slice(),"], "owner admin facet port implementation");

// ── Requests: codes travel in panel order, facets never paginate ─────────────────────────────────
requireAll(
  ownerPortTypes,
  [
    "pub struct StorefrontCatalogFacetsRequest {",
    "pub struct AdminCatalogFacetsRequest {",
    "the pagination fields of `query` are",
    "/// ignored by the owner.",
  ],
  "owner facet requests",
);
const storefrontFacetRequest = bodyAfter(
  ownerPortTypes,
  "pub struct StorefrontCatalogFacetsRequest {",
  "owner storefront facet request",
);
const adminFacetRequest = bodyAfter(
  ownerPortTypes,
  "pub struct AdminCatalogFacetsRequest {",
  "owner admin facet request",
);
for (const [request, name] of [
  [storefrontFacetRequest, "StorefrontCatalogFacetsRequest"],
  [adminFacetRequest, "AdminCatalogFacetsRequest"],
]) {
  requireAll(
    request,
    [
      "pub locale: Option<String>,",
      "pub fallback_locale: Option<String>,",
      "pub query:",
      "/// Attribute codes to count, in display order; empty means \"no facets\".",
      "pub facet_codes: Vec<String>,",
    ],
    `${name} fields`,
  );
  reject(
    request,
    ["pub page:", "pub per_page:"],
    `${name} must not carry pagination: facets count the whole filtered catalog`,
  );
}
requireAll(
  storefrontFacetRequest,
  ["pub public_channel_slug: Option<String>,"],
  "StorefrontCatalogFacetsRequest fields",
);

// ── Owner DTO: one facet shape for both scopes, counters and truncation included ────────────────
requireAll(
  ownerFacets,
  [
    "pub struct StorefrontCatalogFacetValue {",
    "pub struct StorefrontCatalogFacet {",
    "pub const MAX_CATALOG_FACETS: usize = MAX_ATTRIBUTE_FILTERS;",
    "pub const MAX_CATALOG_FACET_VALUES: usize = ",
  ],
  "owner facet DTO",
);
const ownerFacetDto = bodyAfter(ownerFacets, "pub struct StorefrontCatalogFacet {", "owner facet DTO");
requireAll(
  ownerFacetDto,
  [
    "pub code: String,",
    "pub label: String,",
    "pub value_type: String,",
    "pub is_localized: bool,",
    "pub is_enumerable: bool,",
    "pub is_truncated: bool,",
    "pub total_products: u64,",
    "pub values: Vec<StorefrontCatalogFacetValue>,",
  ],
  "owner facet DTO fields",
);

// ── GraphQL contract: two scopes, one owner DTO, every field on the wire ────────────────────────
const FACE_FIELDS = [
  "pub code: String,",
  "pub label: String,",
  "pub value_type: String,",
  "pub is_localized: bool,",
  "pub is_enumerable: bool,",
  "pub is_truncated: bool,",
  "pub total_products: u64,",
];
const BUCKET_FIELDS = ["pub value: String,", "pub label: String,", "pub count: u64,"];
requireAll(
  commerceFacets,
  [
    "pub struct GqlStorefrontCatalogFacetValue {",
    "pub struct GqlStorefrontCatalogFacet {",
    "pub struct GqlAdminCatalogFacetValue {",
    "pub struct GqlAdminCatalogFacet {",
    "impl From<StorefrontCatalogFacet> for GqlStorefrontCatalogFacet {",
    "impl From<StorefrontCatalogFacet> for GqlAdminCatalogFacet {",
  ],
  "commerce GraphQL facet contract",
);
for (const [declaration, nested, description] of [
  [
    "pub struct GqlStorefrontCatalogFacet {",
    "pub values: Vec<GqlStorefrontCatalogFacetValue>,",
    "GqlStorefrontCatalogFacet",
  ],
  ["pub struct GqlAdminCatalogFacet {", "pub values: Vec<GqlAdminCatalogFacetValue>,", "GqlAdminCatalogFacet"],
]) {
  requireAll(bodyAfter(commerceFacets, declaration, description), [...FACE_FIELDS, nested], `${description} fields`);
}
for (const [declaration, description] of [
  ["pub struct GqlStorefrontCatalogFacetValue {", "GqlStorefrontCatalogFacetValue"],
  ["pub struct GqlAdminCatalogFacetValue {", "GqlAdminCatalogFacetValue"],
]) {
  requireAll(bodyAfter(commerceFacets, declaration, description), BUCKET_FIELDS, `${description} fields`);
}

const storefrontRoot = sliceBetween(
  commerceFacets,
  "async fn storefront_product_catalog_facets(",
  "async fn admin_product_catalog_facets(",
  "GraphQL storefront facet root",
);
const adminRoot = sliceBetween(
  commerceFacets,
  "async fn admin_product_catalog_facets(",
  "async fn admin_product_catalog(",
  "GraphQL admin facet root",
);
requireAll(
  storefrontRoot,
  [
    "require_module_enabled(ctx, PRODUCT_MODULE_SLUG).await?;",
    "require_storefront_channel_enabled(ctx).await?;",
    'PortActor::service("commerce-storefront-graphql")',
    '"commerce-graphql-product:storefront-catalog-facets"',
    "StorefrontProductListQuery::try_new_with_attribute_filters(",
    "load_storefront_catalog_facets(",
    "StorefrontCatalogFacetsRequest {",
    "facet_codes,",
    "with_deadline(std::time::Duration::from_secs(2))",
    '"storefront_product_catalog_facets",',
  ],
  "GraphQL storefront facet root",
);
requireAll(
  adminRoot,
  [
    "require_module_enabled(ctx, PRODUCT_MODULE_SLUG).await?;",
    "require_commerce_permission(",
    "&[Permission::PRODUCTS_LIST, Permission::PRODUCTS_READ],",
    "let tenant_id = product_query_tenant(ctx, tenant_id)?;",
    "PortActor::user(auth.user_id.to_string())",
    '"commerce-graphql-product:admin-catalog-facets"',
    "AdminProductListQuery::try_from_transport_with_attribute_filters(",
    "load_admin_catalog_facets(",
    "AdminCatalogFacetsRequest {",
    "facet_codes,",
    "with_deadline(std::time::Duration::from_secs(2))",
    '"admin_product_catalog_facets",',
  ],
  "GraphQL admin facet root",
);
// The scopes are not interchangeable: the storefront root may not demand an admin permission and
// the admin root may not be gated by the public channel.
reject(
  storefrontRoot,
  ["require_commerce_permission(", "product_query_tenant("],
  "GraphQL storefront facet root must stay reachable without admin permissions",
);
reject(
  adminRoot,
  ["require_storefront_channel_enabled("],
  "GraphQL admin facet root must not be gated by a storefront channel",
);
for (const [root, description] of [
  [storefrontRoot, "GraphQL storefront facet root"],
  [adminRoot, "GraphQL admin facet root"],
]) {
  requireAll(
    root,
    [
      ".map(|value| value.trim().to_string())",
      ".or_else(|| request_context.map(|context| context.locale.clone()))",
      ".unwrap_or_else(|| tenant.default_locale.clone());",
      "product_catalog_port_error(",
    ],
    `${description} locale fallback and error mapping`,
  );
}

// ── Transports: both hosts ask for exactly the contract, on both paths ──────────────────────────
const FACET_SELECTION_BODY =
  "code label valueType isLocalized isEnumerable isTruncated totalProducts values { value label count }";
requireAll(
  storefrontGraphql,
  [
    "STOREFRONT_CATALOG_FACETS_QUERY",
    "storefrontProductCatalogFacets(locale: $locale, filter: $filter, facetCodes: $facetCodes)",
    FACET_SELECTION_BODY,
    '#[serde(rename = "storefrontProductCatalogFacets")]',
    "pub async fn fetch_catalog_facets(",
  ],
  "storefront GraphQL facet transport",
);
requireAll(
  adminGraphql,
  [
    "ADMIN_PRODUCT_CATALOG_FACETS_QUERY",
    "adminProductCatalogFacets(tenantId: $tenantId, locale: $locale, filter: $filter, facetCodes: $facetCodes)",
    FACET_SELECTION_BODY,
    '#[serde(rename = "adminProductCatalogFacets")]',
    "facet_codes: Vec<String>,",
  ],
  "admin GraphQL facet transport",
);
// Face counts describe the whole filtered catalog on both hosts, so the facet request of a list
// transport must never carry a page.
requireAll(
  adminGraphql,
  ["// Facets describe the whole filtered catalog, never one page."],
  "admin GraphQL facet transport pagination",
);
requireLine(adminGraphql, /^\s*page: None,$/m, "admin GraphQL facet transport pagination", "page: None,");
requireLine(
  adminGraphql,
  /^\s*per_page: None,$/m,
  "admin GraphQL facet transport pagination",
  "per_page: None,",
);
const storefrontFacetFilter = sliceBetween(
  storefrontGraphql,
  "fn catalog_facets_filter(",
  "pub async fn fetch_catalog_facets(",
  "storefront facet filter builder",
);
requireAll(
  storefrontFacetFilter,
  [
    "search: controls.search.clone(),",
    "category_id: controls.category_id.clone(),",
    "attribute_filters: controls.attribute_filters.clone(),",
  ],
  "storefront facet filter builder",
);
requireLine(
  storefrontFacetFilter,
  /^\s*page: None,$/m,
  "storefront facet filter builder",
  "page: None,",
);
requireLine(
  storefrontFacetFilter,
  /^\s*per_page: None,$/m,
  "storefront facet filter builder",
  "per_page: None,",
);
requireAll(
  adminGraphql,
  ["attribute_filters: controls.attribute_filters,"],
  "admin facet filter variables",
);

// Both transports of one host reach the same owner call, so the panel cannot tell the paths apart.
requireAll(
  storefrontNative,
  [
    'endpoint = "product/storefront/catalog-facets"',
    "fn map_owner_catalog_facets(",
    "facet.is_truncated",
    "facet.total_products",
    "value.count",
    ".storefront_catalog_facets(",
    "facet_codes: Vec<String>,",
    "pub async fn fetch_catalog_facets(",
  ],
  "storefront native facet transport",
);
requireAll(
  adminNative,
  [
    'endpoint = "product/admin/catalog-facets"',
    "fn map_admin_catalog_facets(",
    "facet.is_truncated",
    "facet.total_products",
    "value.count",
    ".admin_catalog_facets(",
    "facet_codes: Vec<String>,",
    "pub(crate) async fn fetch_admin_catalog_facets(",
  ],
  "admin native facet transport",
);
// The native admin endpoint carries the same tenant guard and permission gate as the GraphQL root.
requireAll(
  adminNative,
  [
    "if !rustok_api::has_any_effective_permission(",
    "rustok_api::Permission::PRODUCTS_LIST,",
    "rustok_api::Permission::PRODUCTS_READ,",
    '"tenant_id does not match current tenant"',
    "AdminProductListQuery::try_from_transport_with_attribute_filters(",
    "request_context.map(|context| context.locale)",
  ],
  "admin native facet endpoint guards",
);
requireAll(
  storefrontTransport,
  [
    "pub async fn fetch_catalog_facets(",
    "catalog_facets_native::fetch_catalog_facets(",
    "graphql_adapter::fetch_catalog_facets(Some(locale), controls, facet_codes)",
  ],
  "storefront facet transport selection",
);
requireAll(
  adminTransport,
  [
    "pub(crate) async fn fetch_admin_catalog_facets(",
    "legacy::admin_catalog_native::fetch_admin_catalog_facets(",
    "legacy::admin_catalog_graphql::fetch_facets(",
  ],
  "admin facet transport selection",
);

// ── Panels: the codes each host asks for come from the options it renders ───────────────────────
requireAll(
  storefrontCore,
  [
    "pub fn build_catalog_facet_codes(options: &ProductCatalogSearchOptions) -> Vec<String> {",
    "let code = option.value.trim();",
    "codes.iter().any(|existing| existing == code)",
  ],
  "storefront facet code builder",
);
requireAll(
  adminFacets,
  [
    "pub fn build_product_admin_facet_codes(",
    "let code =",
    "codes.iter().any(|existing| existing == code)",
  ],
  "admin facet code builder",
);
requireAll(
  nextFacets,
  [
    "export function buildCatalogFacetCodes(",
    "const code = (option.value ?? \"\").trim();",
    "codes.includes(code)",
  ],
  "next facet code builder",
);
requireAll(
  storefrontUi,
  [
    "build_catalog_facet_codes(&options)",
    "transport::fetch_catalog_facets(",
  ],
  "leptos storefront facet request",
);
requireAll(
  adminUi,
  [
    "let facet_codes = crate::facets::build_product_admin_facet_codes(&options);",
    "if facet_codes.is_empty() {",
    "catalog_transport::fetch_admin_catalog_facets(",
  ],
  "leptos admin facet request",
);
requireAll(
  nextProducts,
  [
    "export async function fetchStorefrontCatalogFacets(",
    "const codes = facetCodes",
    ".map((code) => code.trim())",
    ".filter((code) => code.length > 0);",
    "if (codes.length === 0) return [];",
    "facetCodes: codes,",
  ],
  "next storefront facet request",
);
requireAll(
  nextPage,
  [
    "fetchStorefrontCatalogFacets(",
    "buildCatalogFacetCodes(searchOptions),",
    "facets = [];",
  ],
  "next catalog page facet request",
);

// ── Caps: a panel may never ask the owner for more facets than it can count ─────────────────────
const ownerFacetCap = ownerFacets.includes(
  "pub const MAX_CATALOG_FACETS: usize = MAX_ATTRIBUTE_FILTERS;",
)
  ? capture(ownerFilterLimits, /MAX_ATTRIBUTE_FILTERS: usize = (\d+);/)
  : undefined;
const ownerValueCap = capture(ownerFacets, /pub const MAX_CATALOG_FACET_VALUES: usize = (\d+);/);
const gridFacetCap = capture(gridFacetRs, /pub const MAX_GRID_FACETS: usize = (\d+);/);
const gridValueCap = capture(gridFacetRs, /pub const MAX_GRID_FACET_VALUES: usize = (\d+);/);
const tsFacetCap = capture(gridFacetTs, /export const MAX_GRID_FACETS = (\d+);/);
const tsValueCap = capture(gridFacetTs, /export const MAX_GRID_FACET_VALUES = (\d+);/);
if (ownerFacetCap === undefined) {
  failures.push(`${OWNER_FACETS}: MAX_CATALOG_FACETS must be derived from the attribute-filter cap`);
}
if (ownerValueCap === undefined) {
  failures.push(`${OWNER_FACETS}: MAX_CATALOG_FACET_VALUES must be a literal`);
}
if (gridFacetCap === undefined || gridValueCap === undefined) {
  failures.push(`${GRID_FACET_RS}: grid facet caps must be literals`);
}
if (tsFacetCap === undefined || tsValueCap === undefined) {
  failures.push(`${GRID_FACET_TS}: the TypeScript facet twin must mirror both caps`);
}
if (ownerFacetCap !== undefined && gridFacetCap !== undefined && ownerFacetCap !== gridFacetCap) {
  failures.push(
    `${GRID_FACET_RS}: the grid renders ${gridFacetCap} facets while the owner counts at most ${ownerFacetCap}; a panel must not ask for a facet it cannot show`,
  );
}
if (ownerValueCap !== undefined && gridValueCap !== undefined && ownerValueCap !== gridValueCap) {
  failures.push(
    `${GRID_FACET_RS}: the grid keeps ${gridValueCap} bucket values per facet while the owner sends up to ${ownerValueCap}`,
  );
}
if (gridFacetCap !== tsFacetCap || gridValueCap !== tsValueCap) {
  failures.push(
    `${GRID_FACET_TS}: the TypeScript facet caps (${tsFacetCap}/${tsValueCap}) must mirror the Rust grid caps (${gridFacetCap}/${gridValueCap})`,
  );
}

// ── Counters and truncation survive the chain ───────────────────────────────────────────────────
requireAll(
  storefrontModel,
  [
    "pub struct ProductCatalogFacet {",
    '#[serde(rename = "isTruncated")]',
    '#[serde(rename = "totalProducts")]',
    "pub is_truncated: bool,",
    "pub total_products: u64,",
  ],
  "storefront facet model",
);
requireAll(
  adminModel,
  [
    "pub struct AdminCatalogFacet {",
    '#[serde(rename = "isTruncated")]',
    '#[serde(rename = "totalProducts")]',
    "pub is_truncated: bool,",
    "pub total_products: u64,",
  ],
  "admin facet model",
);
requireAll(
  storefrontCore,
  ["mapped.is_truncated |= facet.is_truncated;", "facet.total_products,", "FacetValue::new("],
  "shared grid facet mapping (storefront)",
);
requireAll(
  adminFacets,
  ["mapped.is_truncated |= facet.is_truncated;", "facet.total_products,", "FacetValue::new("],
  "shared grid facet mapping (admin)",
);
// The shared builder keeps an empty bucket list for an unbounded domain on both sides of the twin
// contract; a TS-only twin that keeps the buckets would let the Next panel render buckets for an
// attribute the owner deliberately reported as non-enumerable.
requireAll(
  gridFacetRs,
  ["let (values, is_truncated) = if domain.is_enumerable() {", "(Vec::new(), false)"],
  "shared grid facet builder (rust)",
);
requireAll(
  gridFacetTs,
  [
    "const isEnumerable = facetDomainIsEnumerable(input.domain);",
    "if (isEnumerable) {",
    "for (const bucket of input.values ?? []) {",
  ],
  "shared facet builder (typescript twin)",
);
requireAll(
  nextTypes,
  [
    "export type ProductCatalogFacetValue = {",
    "export type ProductCatalogFacet = {",
    "isTruncated: boolean;",
    "totalProducts: number;",
    "values: ProductCatalogFacetValue[];",
  ],
  "next facet types",
);
requireAll(
  nextFacets,
  [
    "export function catalogFacetToSource(",
    "total: facet.totalProducts ?? 0,",
    "values: facet.values ?? []",
    "isTruncated: source.isTruncated || (facet.isTruncated ?? false)",
  ],
  "next facet mapper",
);
requireAll(
  nextFilters,
  [
    "buildCatalogFacetFiltersView(",
    "facets?: ProductCatalogFacet[];",
    "facetPanel.facets.map((facet) =>",
  ],
  "next facet panel",
);
requireAll(
  nextFacets,
  [
    "export function buildCatalogFacetFiltersView(",
    "(facets ?? []).map(catalogFacetToSource)",
    "buildFacetPanel(",
  ],
  "next facet panel view",
);

// ── Boundary: no surface counts facets itself, and no surface crosses scopes ────────────────────
const COUNTING_TABLES = [
  "product_attribute_values",
  "product_attribute_value_options",
  "product_attribute_translations",
  "product_attribute_option_translations",
  "product_attribute_value_translations",
];
for (const [directory, extensions, description, tables] of [
  ["crates/modules/rustok-product/storefront/src", [".rs"], "storefront", COUNTING_TABLES],
  ["crates/modules/rustok-product/admin/src", [".rs"], "admin", []],
  [
    "apps/next-frontend/packages/rustok-product/src",
    [".ts", ".tsx"],
    "next storefront package",
    COUNTING_TABLES,
  ],
]) {
  for (const { file, source } of tree(directory, extensions)) {
    if (source.includes("COUNT(")) {
      failures.push(`${file}: the ${description} must not count facets itself; the owner counts them`);
    }
    for (const table of tables) {
      if (source.includes(table)) {
        failures.push(`${file}: the ${description} must not read ${table}; the owner resolves facet rows`);
      }
    }
  }
}
for (const [directory, forbidden, description] of [
  [
    "crates/modules/rustok-product/storefront/src",
    ["load_admin_catalog_facets", "adminProductCatalogFacets", "admin_catalog_facets"],
    "storefront",
  ],
  [
    "apps/next-frontend/packages/rustok-product/src",
    ["adminProductCatalogFacets", "fetchAdminCatalogFacets"],
    "next storefront package",
  ],
  [
    "crates/modules/rustok-product/admin/src",
    ["load_storefront_catalog_facets", "storefrontProductCatalogFacets", "storefront_catalog_facets"],
    "admin",
  ],
]) {
  const extensions = directory.includes("apps/") ? [".ts", ".tsx"] : [".rs"];
  for (const { file, source } of tree(directory, extensions)) {
    for (const marker of forbidden) {
      if (source.includes(marker)) {
        failures.push(`${file}: the ${description} must use its own facet scope, not ${marker}`);
      }
    }
  }
}

if (failures.length > 0) {
  console.error("product facet chain verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("product facet chain verification passed");
