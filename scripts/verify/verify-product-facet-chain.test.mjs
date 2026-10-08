#!/usr/bin/env node
/**
 * Contract tests for the cross-layer facet chain.
 *
 * The gate is a source verifier, so these tests do two things. First they execute the real Next facet
 * glue (`catalogFacetToSource`, `buildCatalogFacetCodes`) next to the shared toolkit it delegates
 * to, because "the panel shows the owner's numbers" is a behavioural claim, not a textual one: an
 * owner-truncated bucket list must survive the mapper and a value list wider than the grid cap must
 * be reported as truncated. Then they pin the seams that no runtime test can reach — the GraphQL
 * selection of each host must be exactly the field list of the corresponding Rust facet type, the
 * serde names of both models must spell the same contract, and each panel must ask for the codes it
 * renders.
 *
 * The gate itself is exercised against fixtures: every rule the verifier states gets a fixture that
 * breaks it, so a rule that silently stops firing fails the tests rather than passing the sweep.
 */

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { writeWorkspacePackageResolver } from "./lib/workspace-package-resolver.mjs";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const gatePath = path.join(scriptDir, "verify-product-facet-chain.mjs");
const read = (...segments) => readFileSync(path.join(repoRoot, ...segments), "utf8");

const OWNER_PORT = "crates/modules/rustok-product/src/ports/catalog_read.rs";
const OWNER_PORT_TYPES = "crates/modules/rustok-product/src/ports/types.rs";
const OWNER_FACETS = "crates/modules/rustok-product/src/services/catalog/facets.rs";
const COMMERCE_FACETS = "crates/modules/rustok-commerce/src/graphql/product_catalog.rs";
const STOREFRONT_MODEL = "crates/modules/rustok-product/storefront/src/model.rs";
const STOREFRONT_CORE = "crates/modules/rustok-product/storefront/src/core.rs";
const STOREFRONT_UI = "crates/modules/rustok-product/storefront/src/ui/leptos.rs";
const ADMIN_MODEL = "crates/modules/rustok-product/admin/src/model.rs";
const STOREFRONT_GRAPHQL =
  "crates/modules/rustok-product/storefront/src/transport/graphql_adapter.rs";
const ADMIN_GRAPHQL = "crates/modules/rustok-product/admin/src/transport/admin_catalog_graphql.rs";
const STOREFRONT_NATIVE =
  "crates/modules/rustok-product/storefront/src/transport/catalog_facets_native.rs";
const ADMIN_NATIVE = "crates/modules/rustok-product/admin/src/transport/admin_catalog_native.rs";
const GRID_FACET_RS = "crates/ui/rustok-grid/src/facet.rs";
const GRID_FACET_TS = "packages/rustok-ui-grid/src/facet.ts";
const NEXT_TYPES = "apps/next-frontend/packages/rustok-product/src/api/types.ts";
const NEXT_PRODUCTS = "apps/next-frontend/packages/rustok-product/src/api/products.ts";
const NEXT_FACETS = "apps/next-frontend/packages/rustok-product/src/catalog/facets.ts";
const OWNER_FILTER_LIMITS = "crates/modules/rustok-product/src/services/catalog/types.rs";
const OWNER_ATTRIBUTE_FILTERS =
  "crates/modules/rustok-product/src/services/catalog/attribute_filters.rs";
const OWNER_TERM_RESOLUTION =
  "crates/modules/rustok-product/src/services/catalog_schema_service/attributes.rs";
const OWNER_STOREFRONT_LIST = "crates/modules/rustok-product/src/services/catalog/queries.rs";
const OWNER_ADMIN_LIST = "crates/modules/rustok-product/src/services/catalog/admin_queries.rs";
const DISTRIBUTION_SHADOW =
  "crates/modules/rustok-distribution/src/product_index/storefront_shadow.rs";
const OWNER_FACET_POSTGRES_TEST =
  "crates/modules/rustok-product/tests/postgres_facet_counts.rs";

const SELECTION_BODY =
  "code label valueType isLocalized isEnumerable isTruncated totalProducts values { value label count }";

// ── helpers ─────────────────────────────────────────────────────────────────────────────────────

/** Body of the first `{…}` group after `marker`, brace-balanced. */
function braceBodyAfter(source, marker) {
  const start = source.indexOf(marker);
  assert.notEqual(start, -1, `marker ${marker} not found`);
  const open = source.indexOf("{", start);
  assert.notEqual(open, -1, `marker ${marker} has no selection set`);
  let depth = 0;
  for (let index = open; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(open + 1, index);
    }
  }
  throw new Error(`marker ${marker} selection set is not closed`);
}

/** Fields of a Rust struct declaration, in declaration order. */
function structFields(source, declaration) {
  const body = braceBodyAfter(source, declaration);
  return [...body.matchAll(/^\s*pub\s+([a-z_][a-z0-9_]*):/gm)].map((match) => match[1]);
}

/**
 * `field: value` names of a Rust struct literal, in declaration order, ignoring the keys of nested
 * struct literals (the bucket list of a facet is compared through the model, not the literal).
 */
function literalFields(source, marker) {
  const body = braceBodyAfter(source, marker).replace(
    /\s*[A-Za-z_][A-Za-z0-9_]*\s*\{[^}]*\}/g,
    "",
  );
  return [...body.matchAll(/^\s*([a-z_][a-z0-9_]*):/gm)].map((match) => match[1]);
}

const camel = (name) => name.replace(/_([a-z0-9])/g, (_, letter) => letter.toUpperCase());

/** `#[serde(rename = "…")]` name declared for a field of a Rust struct. */
function serdeRename(source, declaration, field) {
  const body = braceBodyAfter(source, declaration);
  const index = body.indexOf(`pub ${field}:`);
  assert.notEqual(index, -1, `${declaration} has no field ${field}`);
  const slice = body.slice(0, index);
  const renames = [...slice.matchAll(/#\[serde\(rename = "([^"]+)"\)\]/g)];
  return renames.length > 0 ? renames[renames.length - 1][1] : field;
}

/** Shape of a facet selection set: scalar fields, nested collection name and its fields. */
function selectionShape(source, marker) {
  const body = braceBodyAfter(source, marker);
  const nested = body.match(/([A-Za-z][A-Za-z0-9]*)\s*\{([^}]*)\}/);
  assert.ok(nested, `selection ${marker} has no nested bucket block`);
  return {
    scalars: body
      .replace(/\s*[A-Za-z][A-Za-z0-9]*\s*\{[^}]*\}/, "")
      .trim()
      .split(/\s+/),
    nestedName: nested[1],
    nested: nested[2].trim().split(/\s+/),
  };
}

/** Extracts `program`-driven checks from the Next facet glue in a child Node process. */
function runNextFacets(body) {
  const harnessDirectory = mkdtempSync(path.join(tmpdir(), "rustok-facet-chain-"));
  const harness = path.join(harnessDirectory, "harness.mjs");
  writeFileSync(
    harness,
    [
      `import * as facets from ${JSON.stringify(
        path.join(repoRoot, "apps/next-frontend/packages/rustok-product/src/catalog/facets.ts"),
      )};`,
      `import * as grid from "@rustok/ui-grid";`,
      "const checks = {};",
      body,
      "process.stdout.write(JSON.stringify(checks));",
      "",
    ].join("\n"),
  );
  const register = writeWorkspacePackageResolver(harnessDirectory, {
    packageName: "@rustok/ui-grid",
    packageRoot: path.join(repoRoot, "packages/rustok-ui-grid"),
    id: "facet-chain",
  });
  try {
    const result = spawnSync(
      process.execPath,
      ["--experimental-strip-types", "--import", register, harness],
      { encoding: "utf8" },
    );
    assert.equal(result.status, 0, `facet harness failed: ${result.stderr || result.stdout}`);
    return JSON.parse(result.stdout);
  } finally {
    rmSync(harnessDirectory, { recursive: true, force: true });
  }
}

// ── gate fixtures ───────────────────────────────────────────────────────────────────────────────

function write(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

const OWNER_PORT_SOURCE = (options) => `
const LOAD_STOREFRONT_CATALOG_FACETS_OPERATION: &str = "load_storefront_catalog_facets";
const LOAD_ADMIN_CATALOG_FACETS_OPERATION: &str = "load_admin_catalog_facets";
trait CatalogReadPort {
    async fn load_storefront_catalog_facets(&self, _context: PortContext, _request: StorefrontCatalogFacetsRequest) -> Result<Vec<StorefrontCatalogFacet>, PortError> {
        Err(PortError::unavailable(
            "product.catalog_facets_unavailable",
            "product catalog facets are unavailable",
        ))
    }
    async fn load_admin_catalog_facets(&self, _context: PortContext, _request: AdminCatalogFacetsRequest) -> Result<Vec<StorefrontCatalogFacet>, PortError> {
        Err(PortError::unavailable(
            "product.admin_catalog_facets_unavailable",
            "product admin catalog facets are unavailable",
        ))
    }
}
impl CatalogReadPort for DatabaseConnection {
    async fn load_storefront_catalog_facets(&self, context: PortContext, request: StorefrontCatalogFacetsRequest) -> Result<Vec<StorefrontCatalogFacet>, PortError> {
        let owner_operation = LOAD_STOREFRONT_CATALOG_FACETS_OPERATION;
        context.require_policy(PortCallPolicy::read())?;
        let locale = request.locale.as_deref().unwrap_or(context.locale.as_str());
        crate::CatalogService::storefront_catalog_facets(
            self,
            locale,
            request.fallback_locale.as_deref(),
            request.public_channel_slug.as_deref(),
            &request.query,
            request.facet_codes.as_slice(),
        )
    }
    async fn load_admin_catalog_facets(&self, context: PortContext, request: AdminCatalogFacetsRequest) -> Result<Vec<StorefrontCatalogFacet>, PortError> {
        let owner_operation = LOAD_ADMIN_CATALOG_FACETS_OPERATION;
        context.require_policy(PortCallPolicy::read())?;
        let locale = request.locale.as_deref().unwrap_or(context.locale.as_str());
        crate::CatalogService::admin_catalog_facets(
            self,
            locale,
            request.fallback_locale.as_deref(),
            ${options.adminPassesChannel ? "request.public_channel_slug.as_deref()," : ""}
            &request.query,
            request.facet_codes.as_slice(),
        )
    }
    async fn read_storefront_product_projection(&self) {}
}
`;

const OWNER_PORT_TYPES_SOURCE = (options) => `
/// Facets are counted for the whole filtered catalog, so the pagination fields of \`query\` are
/// ignored by the owner.
pub struct StorefrontCatalogFacetsRequest {
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub public_channel_slug: Option<String>,
    pub query: StorefrontProductListQuery,
    /// Attribute codes to count, in display order; empty means "no facets".
    pub facet_codes: Vec<String>,
}
/// Facets are counted for the whole filtered admin catalog, so the pagination fields of \`query\` are
/// ignored by the owner.
pub struct AdminCatalogFacetsRequest {
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub query: AdminProductListQuery,
    /// Attribute codes to count, in display order; empty means "no facets".
    pub facet_codes: Vec<String>,
    ${options.adminRequestPaginates ? "pub per_page: u64," : ""}
}
`;

const FACET_POPULATION_SOURCE = `async fn facet_population(filters: &[ProductAttributeFilter]) -> CommerceResult<Condition> {
    // The facet counter resolves the panel selection through the same helper as both lists.
    let other_conditions = load_catalog_attribute_filter_conditions(filters).await?;
    Ok(Condition::all())
}
`;

const OWNER_FACETS_SOURCE = (options) => `
pub const MAX_CATALOG_FACETS: usize = MAX_ATTRIBUTE_FILTERS;
pub const MAX_CATALOG_FACET_VALUES: usize = 20;
pub struct StorefrontCatalogFacetValue {
    pub value: String,
    pub label: String,
    pub count: u64,
}
pub struct StorefrontCatalogFacet {
    pub code: String,
    pub label: String,
    pub value_type: String,
    pub is_localized: bool,
    pub is_enumerable: bool,
    pub is_truncated: bool,
    pub values: Vec<StorefrontCatalogFacetValue>,
    pub total_products: u64,
}
${options.facetPathSkipsSelection ? "" : FACET_POPULATION_SOURCE}`;

const COMMERCE_FACETS_SOURCE = (options) => `
pub struct GqlStorefrontCatalogFacetValue {
    pub value: String,
    pub label: String,
    pub count: u64,
}
pub struct GqlStorefrontCatalogFacet {
    pub code: String,
    pub label: String,
    pub value_type: String,
    pub is_localized: bool,
    pub is_enumerable: bool,
    pub is_truncated: bool,
    pub total_products: u64,
    pub values: Vec<GqlStorefrontCatalogFacetValue>,
}
impl From<StorefrontCatalogFacet> for GqlStorefrontCatalogFacet {
    fn from(facet: StorefrontCatalogFacet) -> Self {
        Self {
            code: facet.code,
            is_truncated: facet.is_truncated,
            total_products: facet.total_products,
        }
    }
}
pub struct GqlAdminCatalogFacetValue {
    pub value: String,
    pub label: String,
    pub count: u64,
}
pub struct GqlAdminCatalogFacet {
    pub code: String,
    pub label: String,
    pub value_type: String,
    pub is_localized: bool,
    pub is_enumerable: bool,
    pub is_truncated: bool,
    pub total_products: u64,
    pub values: Vec<GqlAdminCatalogFacetValue>,
}
impl From<StorefrontCatalogFacet> for GqlAdminCatalogFacet {
    fn from(facet: StorefrontCatalogFacet) -> Self {
        Self {
            code: facet.code,
            is_truncated: facet.is_truncated,
            total_products: facet.total_products,
        }
    }
}
impl ProductQuery {
    async fn storefront_product_catalog_facets(&self) -> Result<Vec<GqlStorefrontCatalogFacet>> {
        require_module_enabled(ctx, PRODUCT_MODULE_SLUG).await?;
        require_storefront_channel_enabled(ctx).await?;
        let requested_locale = locale
            .map(|value| value.trim().to_string())
            .or_else(|| request_context.map(|context| context.locale.clone()))
            .unwrap_or_else(|| tenant.default_locale.clone());
        let list_query = StorefrontProductListQuery::try_new_with_attribute_filters(
            filter.search,
            filter.attribute_filters,
        )?;
        let port_context = PortContext::new(
            tenant.id.to_string(),
            PortActor::service("commerce-storefront-graphql"),
            requested_locale.as_str(),
            "commerce-graphql-product:storefront-catalog-facets".to_string(),
        )
        .with_deadline(std::time::Duration::from_secs(2));
        let facets = port
            .load_storefront_catalog_facets(
                port_context.clone(),
                StorefrontCatalogFacetsRequest {
                    locale: Some(requested_locale),
                    fallback_locale: Some(tenant.default_locale.clone()),
                    public_channel_slug,
                    query: list_query,
                    facet_codes,
                },
            )
            .await
            .map_err(|error| {
                product_catalog_port_error(
                    &port_context,
                    error,
                    "storefront_product_catalog_facets",
                )
            })?;
        Ok(facets.into_iter().map(Into::into).collect())
    }
    async fn admin_product_catalog_facets(&self) -> Result<Vec<GqlAdminCatalogFacet>> {
        require_module_enabled(ctx, PRODUCT_MODULE_SLUG).await?;
        let auth = require_commerce_permission(
            ctx,
            &[Permission::PRODUCTS_LIST, Permission::PRODUCTS_READ],
            "Permission denied: products:list or products:read required",
        )?;
        let tenant_id = product_query_tenant(ctx, tenant_id)?;
        let requested_locale = locale
            .map(|value| value.trim().to_string())
            .or_else(|| request_context.map(|context| context.locale.clone()))
            .unwrap_or_else(|| tenant.default_locale.clone());
        let list_query = AdminProductListQuery::try_from_transport_with_attribute_filters(
            filter.search,
            filter.attribute_filters,
        )?;
        let port_context = PortContext::new(
            tenant_id.to_string(),
            PortActor::user(auth.user_id.to_string()),
            requested_locale.as_str(),
            "commerce-graphql-product:admin-catalog-facets".to_string(),
        )
        .with_deadline(std::time::Duration::from_secs(2));
        let facets = port
            .load_admin_catalog_facets(
                port_context.clone(),
                AdminCatalogFacetsRequest {
                    locale: Some(requested_locale),
                    fallback_locale: Some(tenant.default_locale.clone()),
                    query: list_query,
                    facet_codes,
                },
            )
            .await
            .map_err(|error| {
                product_catalog_port_error(
                    &port_context,
                    error,
                    "admin_product_catalog_facets",
                )
            })?;
        Ok(facets.into_iter().map(Into::into).collect())
    }
    async fn admin_product_catalog(&self) {}
}
`;

const STOREFRONT_GRAPHQL_SOURCE = (options) => `
const STOREFRONT_CATALOG_FACETS_QUERY: &str = "query StorefrontCatalogFacets($locale: String, $filter: StorefrontProductCatalogFilter, $facetCodes: [String!]!) { storefrontProductCatalogFacets(locale: $locale, filter: $filter, facetCodes: $facetCodes) { ${
  options.omitStorefrontTruncation ? SELECTION_BODY.replace(" isTruncated", "") : SELECTION_BODY
} } }";
#[derive(Debug, Deserialize)]
struct StorefrontCatalogFacetsResponse {
    #[serde(rename = "storefrontProductCatalogFacets")]
    facets: Vec<ProductCatalogFacet>,
}
fn catalog_facets_filter(controls: &CatalogListInput) -> StorefrontProductsFilter {
    StorefrontProductsFilter {
        search: controls.search.clone(),
        category_id: controls.category_id.clone(),
        attribute_filters: controls.attribute_filters.clone(),
        page: ${options.facetFilterPaginates ? "Some(1)" : "None"},
        per_page: None,
    }
}
pub async fn fetch_catalog_facets(
    locale: Option<String>,
    controls: CatalogListInput,
    facet_codes: Vec<String>,
) -> Result<Vec<ProductCatalogFacet>, ApiError> {
    let filter = catalog_facets_filter(&controls);
    Ok(vec![])
}
`;

const ADMIN_GRAPHQL_SOURCE = (options) => `
const ADMIN_PRODUCT_CATALOG_FACETS_QUERY: &str = "query ProductAdminCatalogFacets($tenantId: UUID!, $locale: String, $filter: AdminProductCatalogFilter, $facetCodes: [String!]!) { adminProductCatalogFacets(tenantId: $tenantId, locale: $locale, filter: $filter, facetCodes: $facetCodes) { ${
  options.omitAdminTruncation ? SELECTION_BODY.replace(" isTruncated", "") : SELECTION_BODY
} } }";
#[derive(Debug, Deserialize)]
struct AdminProductCatalogFacetsResponse {
    ${
      options.omitSerdeRename
        ? ""
        : '#[serde(rename = "adminProductCatalogFacets")]'
    }
    admin_product_catalog_facets: Vec<AdminCatalogFacet>,
}
pub(crate) async fn fetch_facets(
    token: Option<String>,
    controls: ProductAdminListInput,
    facet_codes: Vec<String>,
) -> Result<Vec<AdminCatalogFacet>, GraphqlHttpError> {
    let filter = AdminProductCatalogFilter {
        search: controls.search,
        attribute_filters: controls.attribute_filters,
        // Facets describe the whole filtered catalog, never one page.
        page: None,
        per_page: None,
    };
    let _ = facet_codes;
    Ok(vec![])
}
`;

const STOREFRONT_NATIVE_SOURCE = `
#[server(prefix = "/api/fn", endpoint = "product/storefront/catalog-facets")]
async fn storefront_catalog_facets_native(
    locale: Option<String>,
    attribute_filters: Vec<String>,
    facet_codes: Vec<String>,
) -> Result<Vec<ProductCatalogFacet>, ServerFnError> {
    let facets = CatalogService::new(db, bus)
        .storefront_catalog_facets(tenant.id, locale, fallback, channel, &query, facet_codes)
        .await?;
    Ok(map_owner_catalog_facets(facets))
}
fn map_owner_catalog_facets(facets: Vec<rustok_product::StorefrontCatalogFacet>) -> Vec<ProductCatalogFacet> {
    facets.into_iter().map(|facet| ProductCatalogFacet {
        code: facet.code,
        value_type: facet.value_type,
        is_truncated: facet.is_truncated,
        total_products: facet.total_products,
        values: facet.values.into_iter().map(|value| ProductCatalogFacetValue {
            value: value.value,
            label: value.label,
            count: value.count,
        }).collect(),
    }).collect()
}
pub async fn fetch_catalog_facets(
    locale: Option<String>,
    controls: CatalogListInput,
    facet_codes: Vec<String>,
) -> Result<Vec<ProductCatalogFacet>, ApiError> {
    let _ = (locale, controls, facet_codes);
    Ok(vec![])
}
`;

const ADMIN_NATIVE_SOURCE = `
#[server(prefix = "/api/fn", endpoint = "product/admin/catalog-facets")]
async fn product_admin_catalog_facets_native(
    tenant_id: String,
    locale: Option<String>,
    attribute_filters: Vec<String>,
    facet_codes: Vec<String>,
) -> Result<Vec<crate::model::AdminCatalogFacet>, ServerFnError> {
    if !rustok_api::has_any_effective_permission(
        &auth.permissions,
        &[
            rustok_api::Permission::PRODUCTS_LIST,
            rustok_api::Permission::PRODUCTS_READ,
        ],
    ) {
        return Err(ServerFnError::new("Permission denied"));
    }
    if requested_tenant_id != tenant.id || auth.tenant_id != tenant.id {
        return Err(ServerFnError::new("tenant_id does not match current tenant"));
    }
    let requested_locale = locale
        .map(|value| value.trim().to_string())
        .or_else(|| request_context.map(|context| context.locale))
        .unwrap_or_else(|| tenant.default_locale.clone());
    let list_query = AdminProductListQuery::try_from_transport_with_attribute_filters(
        search,
        status,
        category_id,
        sort_by,
        sort_direction,
        attribute_filters,
    )?;
    let facets = CatalogService::new(db, bus)
        .admin_catalog_facets(tenant.id, locale, fallback, &list_query, facet_codes.as_slice())
        .await?;
    Ok(map_admin_catalog_facets(facets))
}
fn map_admin_catalog_facets(facets: Vec<rustok_product::StorefrontCatalogFacet>) -> Vec<crate::model::AdminCatalogFacet> {
    facets.into_iter().map(|facet| crate::model::AdminCatalogFacet {
        code: facet.code,
        value_type: facet.value_type,
        is_truncated: facet.is_truncated,
        total_products: facet.total_products,
        values: facet.values.into_iter().map(|value| crate::model::AdminCatalogFacetValue {
            value: value.value,
            label: value.label,
            count: value.count,
        }).collect(),
    }).collect()
}
pub(crate) async fn fetch_admin_catalog_facets(
    tenant_id: String,
    locale: Option<String>,
    controls: ProductAdminListInput,
    facet_codes: Vec<String>,
) -> Result<Vec<crate::model::AdminCatalogFacet>, ServerFnError> {
    product_admin_catalog_facets_native(tenant_id, locale, controls, facet_codes).await
}
`;

const RUST_MODEL_SOURCE = (name, valueName, options = {}) => `
pub struct ${valueName} {
    pub value: String,
    pub label: String,
    pub count: u64,
}
pub struct ${name} {
    pub code: String,
    pub label: String,
    #[serde(rename = "valueType")]
    pub value_type: String,
    #[serde(rename = "isLocalized")]
    pub is_localized: bool,
    #[serde(rename = "isEnumerable")]
    pub is_enumerable: bool,
    #[serde(rename = "isTruncated")]
    pub is_truncated: bool,
    #[serde(rename = "totalProducts")]
    pub total_products: u64,
    pub values: Vec<${valueName}>,
    ${
      options.crossScopeMarker
        ? "// shared with the other scope: load_admin_catalog_facets"
        : ""
    }
}
`;

const STOREFRONT_GRID_SOURCE = `
fn catalog_facet_to_grid(facet: &ProductCatalogFacet) -> GridFacet {
    let mut mapped = GridFacet::from_buckets(
        facet.code.as_str(),
        facet.label.as_str(),
        domain,
        facet.total_products,
        facet.values.iter().map(|value| FacetValue::new(value.value.clone(), value.label.clone(), value.count)),
    );
    mapped.is_truncated |= facet.is_truncated;
    mapped
}
`;

const ADMIN_GRID_SOURCE = `
pub fn admin_catalog_facets_to_grid(facets: &[AdminCatalogFacet]) -> Vec<GridFacet> {
    facets.iter().map(|facet| {
        let mut mapped = GridFacet::from_buckets(
            facet.code.as_str(),
            facet.label.as_str(),
            domain,
            facet.total_products,
            facet.values.iter().map(|value| FacetValue::new(value.value.clone(), value.label.clone(), value.count)),
        );
        mapped.is_truncated |= facet.is_truncated;
        mapped
    }).collect()
}
pub fn build_product_admin_facet_codes(options: &ProductCatalogSearchOptions) -> Vec<String> {
    let mut codes: Vec<String> = Vec::new();
    for option in &options.attribute_options {
        let code = option.value.trim();
        if code.is_empty() || codes.iter().any(|existing| existing == code) {
            continue;
        }
        codes.push(code.to_string());
    }
    codes
}
`;

const STOREFRONT_CORE_SOURCE = `
pub fn build_catalog_facet_codes(options: &ProductCatalogSearchOptions) -> Vec<String> {
    let mut codes: Vec<String> = Vec::new();
    for option in &options.attribute_options {
        let code = option.value.trim();
        if code.is_empty() || codes.iter().any(|existing| existing == code) {
            continue;
        }
        codes.push(code.to_string());
    }
    codes
}
fn catalog_facet_to_grid(facet: &ProductCatalogFacet) -> GridFacet {
    let mut mapped = GridFacet::from_buckets(
        facet.code.as_str(),
        facet.label.as_str(),
        domain,
        facet.total_products,
        facet.values.iter().map(|value| FacetValue::new(value.value.clone(), value.label.clone(), value.count)),
    );
    mapped.is_truncated |= facet.is_truncated;
    mapped
}
`;

const STOREFRONT_UI_SOURCE = `
let catalog_support_resource = Resource::new_blocking(
    move || (options_locale.clone(), catalog_input.clone()),
    move |(locale, controls)| async move {
        let options = transport::fetch_catalog_search_options(locale.clone()).await;
        let facets = transport::fetch_catalog_facets(
            locale,
            controls,
            build_catalog_facet_codes(&options),
        )
        .await
        .unwrap_or_default();
        (options, facets)
    },
);
`;

const STOREFRONT_TRANSPORT_SOURCE = `
pub async fn fetch_catalog_facets(
    locale: String,
    controls: CatalogListInput,
    facet_codes: Vec<String>,
) -> TransportResult<Vec<ProductCatalogFacet>> {
    execute_selected_transport(
        "product",
        selected_transport_path(),
        move || catalog_facets_native::fetch_catalog_facets(Some(native_locale), native_controls, native_facet_codes),
        move || async move {
            graphql_adapter::fetch_catalog_facets(Some(locale), controls, facet_codes)
        },
    )
    .await
}
`;

const ADMIN_UI_SOURCE = `
let facet_codes = crate::facets::build_product_admin_facet_codes(&options);
if facet_codes.is_empty() {
    return Some(Vec::new());
}
catalog_transport::fetch_admin_catalog_facets(
    tok,
    ten,
    bootstrap.current_tenant.id,
    loc,
    controls,
    facet_codes,
)
.await
`;

const ADMIN_TRANSPORT_SOURCE = `
pub(crate) async fn fetch_admin_catalog_facets(
    token: Option<String>,
    tenant_id: String,
    locale: Option<String>,
    controls: ProductAdminListInput,
    facet_codes: Vec<String>,
) -> Option<Vec<crate::model::AdminCatalogFacet>> {
    if let Ok(value) = legacy::admin_catalog_native::fetch_admin_catalog_facets(
        tenant_id.clone(),
        locale.clone(),
        controls.clone(),
        facet_codes.clone(),
    )
    .await
    {
        return Some(value);
    }
    legacy::admin_catalog_graphql::fetch_facets(
        token,
        tenant_id,
        locale,
        controls,
        facet_codes,
    )
    .await
    .ok()
}
`;

const NEXT_FACETS_SOURCE = (options) => `
export function buildCatalogFacetCodes(
  options: ProductCatalogSearchOptions,
): string[] {
  const codes: string[] = [];
  for (const option of options.attributeOptions ?? []) {
    const code = (option.value ?? "").trim();
    if (code.length === 0 || codes.includes(code)) continue;
    codes.push(code);
  }
  return codes;
}
export function catalogFacetToSource(
  facet: ProductCatalogFacet,
): FacetSource {
  const domain: FacetDomain = facet.isEnumerable
    ? facet.valueType?.toLowerCase() === "boolean"
      ? { kind: "boolean" }
      : { kind: "dictionary", multi: false }
    : { kind: "open" };
  const source = facetFromBuckets({
    code: facet.code,
    label: facet.label,
    domain,
    total: facet.totalProducts ?? 0,
    values: facet.values ?? []
  });
  return {
    ...source,
    ${
      options.omitNextTruncation
        ? "isTruncated: source.isTruncated"
        : "isTruncated: source.isTruncated || (facet.isTruncated ?? false)"
    }
  };
}
export function buildCatalogFacetFiltersView(
  routeBase: string,
  facets: readonly ProductCatalogFacet[],
  controls: CatalogFacetControls,
  labels: CatalogFacetLabels,
): CatalogFacetFiltersView {
  const panel = buildFacetPanel((facets ?? []).map(catalogFacetToSource), [], labels);
  return { ...panel, facets: panel.facets };
}
`;

const NEXT_PRODUCTS_SOURCE = `
const STOREFRONT_CATALOG_FACETS_QUERY = \`
  query StorefrontCatalogFacets($locale: String, $filter: StorefrontProductCatalogFilter) {
    storefrontProductCatalogFacets {
      code
      label
      valueType
      isLocalized
      isEnumerable
      isTruncated
      totalProducts
      values {
        value
        label
        count
      }
    }
  }
\`;
export async function fetchStorefrontCatalogFacets(
  graphql: ProductGraphqlExecutor,
  locale: string,
  filter: StorefrontCatalogFilter | undefined,
  facetCodes: readonly string[],
  tenantSlug?: string | null,
): Promise<ProductCatalogFacet[]> {
  const codes = facetCodes
    .map((code) => code.trim())
    .filter((code) => code.length > 0);
  if (codes.length === 0) return [];
  const response = await graphql({
    variables: {
      filter: {},
      facetCodes: codes,
    },
  });
  return response.data?.storefrontProductCatalogFacets ?? [];
}
`;

const NEXT_TYPES_SOURCE = `
export type ProductCatalogFacetValue = {
  value: string;
  label: string;
  count: number;
};
export type ProductCatalogFacet = {
  code: string;
  label: string;
  valueType: string;
  isLocalized: boolean;
  isEnumerable: boolean;
  isTruncated: boolean;
  totalProducts: number;
  values: ProductCatalogFacetValue[];
};
`;

const NEXT_FILTERS_SOURCE = `
import { buildCatalogFacetFiltersView } from "../catalog/facets";
interface ProductFiltersProps {
  facets?: ProductCatalogFacet[];
}
export function ProductFilters({ facets = [] }: ProductFiltersProps) {
  const facetPanel = buildCatalogFacetFiltersView("/products", facets, controls, labels);
  return facetPanel.facets.map((facet) => (
    <div key={facet.code}>{facet.label}</div>
  ));
}
`;

const NEXT_PAGE_SOURCE = `
let facets: ProductCatalogFacet[] = [];
try {
  facets = await fetchStorefrontCatalogFacets(
    storefrontGraphql,
    locale,
    filter,
    buildCatalogFacetCodes(searchOptions),
    tenantSlug
  );
} catch {
  facets = [];
}
`;

const OWNER_FILTER_LIMITS_SOURCE = (options) =>
  `
MAX_ATTRIBUTE_FILTERS: usize = 8;
` +
  (options.entryBoundMissing
    ? ""
    : `pub(crate) const MAX_ATTRIBUTE_FILTER_ENTRIES: usize =
    MAX_ATTRIBUTE_FILTERS * super::facets::MAX_CATALOG_FACET_VALUES;
`) +
  `
pub struct ProductAttributeFilterSelection {
    pub code: String,
    pub values: Vec<String>,
}
pub fn group_product_attribute_filters(
    filters: &[ProductAttributeFilter],
) -> Vec<ProductAttributeFilterSelection> {
    let mut selections: Vec<ProductAttributeFilterSelection> = Vec::new();
    for filter in filters {
        match selections
            .iter_mut()
            .find(|selection| selection.code.eq_ignore_ascii_case(filter.code.as_str()))
        {
            Some(selection) => {
                if !selection.values.iter().any(|value| value == &filter.value) {
                    selection.values.push(filter.value.clone());
                }
            }
            None => selections.push(ProductAttributeFilterSelection {
                code: filter.code.clone(),
                values: vec![filter.value.clone()],
            }),
        }
    }
    selections
}
fn validate_product_attribute_filters(filters: &[ProductAttributeFilter]) -> CommerceResult<()> {
${options.entryBoundMissing ? "" : `    if filters.len() > MAX_ATTRIBUTE_FILTER_ENTRIES {
        return Err(CommerceError::Validation(format!(
            "attribute_filters supports at most {MAX_ATTRIBUTE_FILTER_ENTRIES} code=value entries"
        )));
    }
`}    let selection_count = group_product_attribute_filters(filters).len();
    if selection_count > MAX_ATTRIBUTE_FILTERS {
        return Err(CommerceError::Validation(format!(
            "attribute_filters supports at most {MAX_ATTRIBUTE_FILTERS} attributes"
        )));
    }
    Ok(())
}
` + (options.attributeFilterRejectsRepeats ? '                "attribute filter {} occurs more than once",\n' : "");

const OWNER_ATTRIBUTE_FILTERS_SOURCE = (options) => `
pub(super) async fn load_catalog_attribute_filter_conditions(
    filters: &[ProductAttributeFilter],
) -> CommerceResult<Vec<Condition>> {
    let selections = group_product_attribute_filters(filters);
    let mut conditions = Vec::with_capacity(selections.len());
    for selection in &selections {
        let mut attribute_condition = Condition::${options.attributeFiltersConjunction ? "all" : "any"}();
        attribute_condition = attribute_condition.add(build_attribute_filter_condition(selection)?);
        conditions.push(attribute_condition);
    }
    Ok(conditions)
}
`;

const OWNER_TERM_RESOLUTION_SOURCE = (options) => `
pub async fn resolve_storefront_attribute_filter_terms(
    filters: &[ProductAttributeFilter],
) -> CommerceResult<Vec<ProductResolvedAttributeFilter>> {
    let selections = group_product_attribute_filters(filters);
    let mut resolved = Vec::with_capacity(selections.len());
    for selection in &selections {
        let predicate = if predicates.len() == 1 {
            predicates.remove(0)
        } else {
            ProductAttributeTermExpr::Or(predicates)
        };
        resolved.push(ProductResolvedAttributeFilter {
            code: ${options.termResolutionPerEntry ? "filter.code.clone()" : "selection.code.clone()"},
            predicate,
        });
    }
    Ok(resolved)
}
`;

const OWNER_LIST_SOURCE = `
for condition in attribute_filters::load_catalog_attribute_filter_conditions(
    list_query.attribute_filters.as_slice(),
)
.await?
{
    query = query.filter(condition);
}
`;

const OWNER_ADMIN_LIST_SOURCE = (options) =>
  options.adminListSkipsAttributeFilters ? "fn admin_product_list() {}\n" : OWNER_LIST_SOURCE;

const DISTRIBUTION_SHADOW_SOURCE = (options) => `
fn resolved_attribute_filters_to_index(
    owner: &StorefrontProductListQuery,
    resolved: Vec<ProductResolvedAttributeFilter>,
) -> Result<Vec<FilterExpr>, ProductStorefrontIndexShadowError> {
    let selections = group_product_attribute_filters(owner.attribute_filters.as_slice());
    if resolved.len() != ${options.shadowKeepsEntryCount ? "owner.attribute_filters.len()" : "selections.len()"} {
        return Err(ProductStorefrontIndexShadowError::AttributeFilterResolutionMismatch);
    }
    resolved.into_iter().map(product_term_expr_to_index).collect()
}
`;

const OWNER_FACET_POSTGRES_TEST_SOURCE = (options) => `
let widened_own = facet(&widened, "color");
assert_eq!(
    widened_own.total_products, 5,
    "a facet ignores every value of its own selection"
);
` +
  (options.testPinsRejection
    ? `let repeated_code = StorefrontProductListQuery::try_new_with_attribute_filters(
    None, None, None, None,
    vec![format!("color={RED}"), format!("color={BLUE}")],
)
.expect_err("a repeated attribute code is rejected");
assert!(repeated_code.to_string().contains("attribute filter color occurs more than once"));\n`
    : "");

function fixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-facet-chain-"));
  write(root, OWNER_PORT, OWNER_PORT_SOURCE(options));
  write(root, OWNER_PORT_TYPES, OWNER_PORT_TYPES_SOURCE(options));
  write(root, OWNER_FACETS, OWNER_FACETS_SOURCE(options));
  write(root, OWNER_FILTER_LIMITS, OWNER_FILTER_LIMITS_SOURCE(options));
  write(root, OWNER_ATTRIBUTE_FILTERS, OWNER_ATTRIBUTE_FILTERS_SOURCE(options));
  write(root, OWNER_TERM_RESOLUTION, OWNER_TERM_RESOLUTION_SOURCE(options));
  write(root, OWNER_STOREFRONT_LIST, OWNER_LIST_SOURCE);
  write(root, OWNER_ADMIN_LIST, OWNER_ADMIN_LIST_SOURCE(options));
  write(root, DISTRIBUTION_SHADOW, DISTRIBUTION_SHADOW_SOURCE(options));
  write(root, OWNER_FACET_POSTGRES_TEST, OWNER_FACET_POSTGRES_TEST_SOURCE(options));
  write(root, COMMERCE_FACETS, COMMERCE_FACETS_SOURCE(options));
  write(root, STOREFRONT_MODEL, RUST_MODEL_SOURCE("ProductCatalogFacet", "ProductCatalogFacetValue", options));
  write(root, ADMIN_MODEL, RUST_MODEL_SOURCE("AdminCatalogFacet", "AdminCatalogFacetValue", options));
  write(root, STOREFRONT_GRAPHQL, STOREFRONT_GRAPHQL_SOURCE(options));
  write(root, ADMIN_GRAPHQL, ADMIN_GRAPHQL_SOURCE(options));
  write(root, STOREFRONT_NATIVE, STOREFRONT_NATIVE_SOURCE);
  write(root, ADMIN_NATIVE, ADMIN_NATIVE_SOURCE);
  write(root, "crates/modules/rustok-product/storefront/src/transport/mod.rs", STOREFRONT_TRANSPORT_SOURCE);
  write(root, "crates/modules/rustok-product/admin/src/catalog_transport.rs", ADMIN_TRANSPORT_SOURCE);
  write(
    root,
    GRID_FACET_RS,
    `pub const MAX_GRID_FACETS: usize = ${options.gridCapDrift ? 12 : 8};
pub const MAX_GRID_FACET_VALUES: usize = 20;
pub fn from_buckets(domain: FacetDomain, buckets: Vec<FacetValue>) -> Self {
    let (values, is_truncated) = if domain.is_enumerable() {
        (buckets, false)
    } else {
        (Vec::new(), false)
    };
    Self { values, is_truncated }
}
`,
  );
  write(
    root,
    GRID_FACET_TS,
    `export const MAX_GRID_FACETS = ${options.gridCapDrift ? 12 : 8};
export const MAX_GRID_FACET_VALUES = 20;
export function facetFromBuckets(input) {
  const isEnumerable = facetDomainIsEnumerable(input.domain);
  const values = [];
  let isTruncated = false;
  if (isEnumerable) {
    for (const bucket of input.values ?? []) {
      values.push(bucket);
    }
  }
  return { domain: input.domain, isEnumerable, isTruncated, total: input.total, values };
}
`,
  );
  write(root, STOREFRONT_CORE, STOREFRONT_CORE_SOURCE);
  write(root, STOREFRONT_UI, STOREFRONT_UI_SOURCE);
  write(root, "crates/modules/rustok-product/admin/src/facets.rs", ADMIN_GRID_SOURCE);
  write(root, "crates/modules/rustok-product/admin/src/ui/product_grid.rs", ADMIN_UI_SOURCE);
  write(root, NEXT_TYPES, NEXT_TYPES_SOURCE);
  write(root, NEXT_PRODUCTS, NEXT_PRODUCTS_SOURCE);
  write(root, NEXT_FACETS, NEXT_FACETS_SOURCE(options));
  write(root, "apps/next-frontend/packages/rustok-product/src/components/product-filters.tsx", NEXT_FILTERS_SOURCE);
  write(root, "apps/next-frontend/src/app/[locale]/products/page.tsx", NEXT_PAGE_SOURCE);
  if (options.storefrontCounts) {
    write(root, "crates/modules/rustok-product/storefront/src/core.rs", `${STOREFRONT_CORE_SOURCE}
fn broken() { SELECT COUNT(*) FROM product_attribute_values }
`);
  }
  return root;
}

function runGate(root) {
  if (process.env.FACET_DEBUG) console.error("FIXTURE", root);
  return spawnSync(process.execPath, [gatePath], {
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

function expectFailure(options, pattern) {
  const root = fixture(options);
  try {
    const result = runGate(root);
    assert.notEqual(result.status, 0, "the gate must reject the fixture");
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("the gate accepts a fixture shaped like the real chain", () => {
  const root = fixture();
  try {
    const result = runGate(root);
    assert.equal(result.status, 0, result.stderr);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("the gate rejects a selection that stops asking for the truncation flag", () => {
  expectFailure({ omitAdminTruncation: true }, /admin GraphQL facet transport/);
});

test("the gate rejects a host that drops the serde name of the owner root", () => {
  expectFailure(
    { omitSerdeRename: true },
    /admin GraphQL facet transport: missing #\[serde\(rename = "adminProductCatalogFacets"\)\]/,
  );
});

test("the gate rejects a facet request that carries pagination", () => {
  expectFailure(
    { adminRequestPaginates: true },
    /AdminCatalogFacetsRequest must not carry pagination/,
  );
});

test("the gate rejects a facet filter that starts paginating", () => {
  expectFailure(
    { facetFilterPaginates: true },
    /storefront facet filter builder: missing line page: None,/,
  );
});

test("the gate rejects an admin facet call scoped to a public channel", () => {
  expectFailure(
    { adminPassesChannel: true },
    /owner admin facet port implementation must not be scoped to a public channel/,
  );
});

test("the gate rejects a panel whose cap drifted from the owner's", () => {
  expectFailure({ gridCapDrift: true }, /renders 12 facets while the owner counts at most 8/);
});

test("the gate rejects a storefront that reaches the admin facet root", () => {
  expectFailure({ crossScopeMarker: true }, /must use its own facet scope/);
});

test("the gate rejects a surface that counts facets itself", () => {
  expectFailure({ storefrontCounts: true }, /must not count facets itself/);
});

test("the gate rejects a next mapper that drops the owner's truncation", () => {
  expectFailure(
    { omitNextTruncation: true },
    /next facet mapper: missing isTruncated: source\.isTruncated \|\| \(facet\.isTruncated \?\? false\)/,
  );
});

test("the gate rejects an attribute selection that ANDs one attribute's values", () => {
  expectFailure(
    { attributeFiltersConjunction: true },
    /\(one condition per attribute\): missing let mut attribute_condition = Condition::any\(\);/,
  );
});

test("the gate rejects a reintroduced duplicate-code rejection", () => {
  expectFailure(
    { attributeFilterRejectsRepeats: true },
    /services\/catalog\/types\.rs: forbidden occurs more than once/,
  );
});

test("the gate rejects a term resolution that answers one entry per value", () => {
  expectFailure(
    { termResolutionPerEntry: true },
    /catalog_schema_service\/attributes\.rs: forbidden code: filter\.code\.clone\(\),/,
  );
});

test("the gate rejects a next admin list that stops sharing the attribute selection", () => {
  expectFailure(
    { adminListSkipsAttributeFilters: true },
    /admin_queries\.rs \(list and facet paths share the selection\): missing load_catalog_attribute_filter_conditions\(/,
  );
});

test("the gate rejects an index shadow that keys the resolution by entry", () => {
  expectFailure(
    { shadowKeepsEntryCount: true },
    /forbidden resolved\.len\(\) != owner\.attribute_filters\.len\(\)/,
  );
});

test("the gate rejects a selection whose entry bound is derived from the code bound only", () => {
  expectFailure(
    { entryBoundMissing: true },
    /attribute selection contract\): missing pub\(crate\) const MAX_ATTRIBUTE_FILTER_ENTRIES: usize =/,
  );
});

test("the gate rejects a facet counter that resolves the selection on its own", () => {
  expectFailure(
    { facetPathSkipsSelection: true },
    /facets\.rs \(list and facet paths share the selection\): missing load_catalog_attribute_filter_conditions\(/,
  );
});

// ── the real chain ──────────────────────────────────────────────────────────────────────────────

test("both facet roots and both transports request exactly the Rust facet contract", () => {
  const commerce = read(COMMERCE_FACETS);
  const storefrontContract = structFields(commerce, "pub struct GqlStorefrontCatalogFacet {");
  const adminContract = structFields(commerce, "pub struct GqlAdminCatalogFacet {");
  const bucketContract = structFields(commerce, "pub struct GqlStorefrontCatalogFacetValue {");
  assert.deepEqual(adminContract, storefrontContract, "both scopes expose the same facet shape");
  assert.deepEqual(bucketContract, ["value", "label", "count"]);

  const expectedScalars = storefrontContract
    .filter((field) => field !== "values")
    .map(camel);
  assert.deepEqual(expectedScalars, [
    "code",
    "label",
    "valueType",
    "isLocalized",
    "isEnumerable",
    "isTruncated",
    "totalProducts",
  ]);

  const storefrontSelection = selectionShape(
    read(STOREFRONT_GRAPHQL),
    "storefrontProductCatalogFacets(locale: $locale, filter: $filter, facetCodes: $facetCodes)",
  );
  const adminSelection = selectionShape(
    read(ADMIN_GRAPHQL),
    "adminProductCatalogFacets(tenantId: $tenantId, locale: $locale, filter: $filter, facetCodes: $facetCodes)",
  );
  const nextSelection = selectionShape(
    read(NEXT_PRODUCTS),
    "storefrontProductCatalogFacets(\n      locale: $locale,\n      filter: $filter,\n      facetCodes: $facetCodes\n    )",
  );
  for (const [name, selection] of [
    ["leptos storefront", storefrontSelection],
    ["leptos admin", adminSelection],
    ["next storefront", nextSelection],
  ]) {
    assert.deepEqual(selection.scalars, expectedScalars, `${name} selection fields`);
    assert.equal(selection.nestedName, "values", `${name} nested block`);
    assert.deepEqual(selection.nested, bucketContract.map(camel), `${name} bucket fields`);
  }
});

test("the Rust models and the Next types spell the same facet contract", () => {
  const commerce = read(COMMERCE_FACETS);
  const contract = structFields(commerce, "pub struct GqlStorefrontCatalogFacet {")
    .filter((field) => field !== "values")
    .map(camel);

  for (const [path, declaration] of [
    [STOREFRONT_MODEL, "pub struct ProductCatalogFacet {"],
    [ADMIN_MODEL, "pub struct AdminCatalogFacet {"],
  ]) {
    const source = read(path);
    const fields = structFields(source, declaration).filter((field) => field !== "values");
    assert.deepEqual(
      fields.map((field) => serdeRename(source, declaration, field)),
      contract,
      `${path} must serialize as the GraphQL contract`,
    );
  }

  const nextTypes = read(NEXT_TYPES);
  const nextFields = [...nextTypes.matchAll(/^\s{2}([A-Za-z][A-Za-z0-9]*):/gm)].map(
    (match) => match[1],
  );
  for (const field of contract) {
    assert.ok(nextFields.includes(field), `next facet types must carry ${field}`);
  }
});

test("both facet scopes are built from the owner DTO, not from a second counting path", () => {
  const commerce = read(COMMERCE_FACETS);
  assert.match(
    commerce,
    /impl From<StorefrontCatalogFacet> for GqlAdminCatalogFacet \{/,
    "the admin facet must map the owner facet DTO",
  );
  const fromBody = braceBodyAfter(commerce, "impl From<StorefrontCatalogFacet> for GqlAdminCatalogFacet {");
  for (const field of ["is_truncated", "total_products", "values"]) {
    assert.ok(
      fromBody.includes(`${field}: facet.${field}`),
      `the admin facet conversion must forward ${field}`,
    );
  }
  // The owner differ comes from the count scope, so the two services must stay separate entry
  // points: one published-only storefront count and one admin count.
  const ownerFacets = read(OWNER_FACETS);
  assert.match(ownerFacets, /pub async fn storefront_catalog_facets\(/);
  assert.match(ownerFacets, /pub async fn admin_catalog_facets\(/);
  assert.match(
    ownerFacets,
    /fn normalize_facet_codes\(facet_codes: &\[String\]\) -> CommerceResult<Vec<String>> \{/,
    "the owner must normalize the requested codes itself",
  );
  assert.match(ownerFacets, /if codes\.len\(\) == MAX_CATALOG_FACETS \{/);
});

test("the caps of the grid, the panels and the owner agree", () => {
  const ownerFilters = read("crates/modules/rustok-product/src/services/catalog/types.rs");
  const ownerFacets = read(OWNER_FACETS);
  const ownerFacetCap = Number(
    ownerFilters.match(/MAX_ATTRIBUTE_FILTERS: usize = (\d+);/)[1],
  );
  const ownerValueCap = Number(ownerFacets.match(/MAX_CATALOG_FACET_VALUES: usize = (\d+);/)[1]);
  const gridFacetCap = Number(read(GRID_FACET_RS).match(/MAX_GRID_FACETS: usize = (\d+);/)[1]);
  const gridValueCap = Number(
    read(GRID_FACET_RS).match(/MAX_GRID_FACET_VALUES: usize = (\d+);/)[1],
  );
  const tsFacetCap = Number(read(GRID_FACET_TS).match(/export const MAX_GRID_FACETS = (\d+);/)[1]);
  const tsValueCap = Number(
    read(GRID_FACET_TS).match(/export const MAX_GRID_FACET_VALUES = (\d+);/)[1],
  );
  assert.equal(ownerFacetCap, gridFacetCap);
  assert.equal(ownerFacetCap, tsFacetCap);
  assert.equal(ownerValueCap, gridValueCap);
  assert.equal(ownerValueCap, tsValueCap);
  assert.equal(ownerFacetCap, 8);
  assert.equal(ownerValueCap, 20);
});

test("both hosts derive the requested codes from the options they render", () => {
  const storefrontCore = read("crates/modules/rustok-product/storefront/src/core.rs");
  const adminFacets = read("crates/modules/rustok-product/admin/src/facets.rs");
  const nextFacets = read(NEXT_FACETS);
  for (const [source, declaration] of [
    [storefrontCore, "pub fn build_catalog_facet_codes("],
    [adminFacets, "pub fn build_product_admin_facet_codes("],
  ]) {
    const body = braceBodyAfter(source, declaration);
    assert.ok(body.includes(".trim()"), `${declaration} must trim option codes`);
    assert.ok(
      body.includes("codes.iter().any(|existing| existing == code)"),
      `${declaration} must drop duplicate option codes`,
    );
  }
  const nextBody = braceBodyAfter(nextFacets, "export function buildCatalogFacetCodes(");
  assert.ok(nextBody.includes("codes.includes(code)"), "the next builder must drop duplicates");
  assert.ok(nextBody.includes('(option.value ?? "").trim()'), "the next builder must trim codes");

  assert.match(
    read("crates/modules/rustok-product/storefront/src/transport/graphql_adapter.rs"),
    /facet_codes/,
  );
  const storefrontUi = read("crates/modules/rustok-product/storefront/src/ui/leptos.rs");
  assert.match(storefrontUi, /build_catalog_facet_codes\(&options\)/);
  const adminUi = read("crates/modules/rustok-product/admin/src/ui/product_grid.rs");
  assert.match(adminUi, /build_product_admin_facet_codes\(&options\)/);
  assert.match(adminUi, /if facet_codes\.is_empty\(\) \{/);
  assert.match(
    read("apps/next-frontend/src/app/[locale]/products/page.tsx"),
    /buildCatalogFacetCodes\(searchOptions\)/,
  );
  assert.match(read(NEXT_PRODUCTS), /if \(codes\.length === 0\) return \[\];/);
});

test("the native transports forward the owner counters without re-shaping them", () => {
  const storefrontModel = structFields(read(STOREFRONT_MODEL), "pub struct ProductCatalogFacet {");
  const adminModel = structFields(read(ADMIN_MODEL), "pub struct AdminCatalogFacet {");
  assert.deepEqual(adminModel, storefrontModel);
  for (const [path, marker, modelFields] of [
    [STOREFRONT_NATIVE, "|facet| ProductCatalogFacet {", storefrontModel],
    [ADMIN_NATIVE, "|facet| crate::model::AdminCatalogFacet {", adminModel],
  ]) {
    const literal = literalFields(read(path), marker).filter((field) => field !== "values");
    assert.deepEqual(literal, modelFields.filter((field) => field !== "values"), `${path} literal`);
  }
});

test("the shared grid merge keeps the owner's truncation and counts", () => {
  for (const [path, marker] of [
    ["crates/modules/rustok-product/storefront/src/core.rs", "fn catalog_facet_to_grid("],
    ["crates/modules/rustok-product/admin/src/facets.rs", "fn admin_catalog_facets_to_grid("],
  ]) {
    const body = braceBodyAfter(read(path), marker);
    assert.ok(
      body.includes("mapped.is_truncated |= facet.is_truncated;"),
      `${path} must merge the owner's truncation flag`,
    );
    assert.ok(body.includes("facet.total_products"), `${path} must pass the owner's total`);
    assert.ok(body.includes("FacetValue::new("), `${path} must map the owner's buckets`);
  }
});

test("the next mapper preserves owner counts and reports truncation", () => {
  const behaviour = runNextFacets(`
    const facet = (values, extra = {}) => ({
      code: "color",
      label: "Цвет",
      valueType: "select",
      isLocalized: false,
      isEnumerable: true,
      isTruncated: false,
      totalProducts: 42,
      values,
      ...extra
    });
    const buckets = Array.from({ length: 25 }, (_, index) => ({
      value: "v" + index,
      label: "V" + index,
      count: index + 1
    }));
    checks.plain = facets.catalogFacetToSource(facet([{ value: "red", label: "Красный", count: 3 }]));
    checks.wide = facets.catalogFacetToSource(facet(buckets));
    checks.ownerCut = facets.catalogFacetToSource(facet([{ value: "red", label: "Красный", count: 3 }], { isTruncated: true }));
    checks.open = facets.catalogFacetToSource(facet(buckets, { isEnumerable: false }));
    checks.boolean = facets.catalogFacetToSource(facet(
      [{ value: "true", label: "Yes", count: 5 }, { value: "false", label: "No", count: 1 }],
      { valueType: "boolean" }
    ));
    checks.multi = facets.catalogFacetToSource(facet([{ value: "m", label: "M", count: 1 }], { valueType: "multiselect" }));
    checks.codes = facets.buildCatalogFacetCodes({
      attributeOptions: [
        { value: "  color ", label: "Цвет" },
        { value: "", label: "Пусто" },
        { value: "color", label: "Дубль" },
        { value: "size", label: "Размер" }
      ]
    });
    checks.noOptions = facets.buildCatalogFacetCodes({});
    checks.shared = grid.facetFromBuckets({
      code: "color",
      label: "Цвет",
      domain: { kind: "dictionary", multi: false },
      total: 42,
      values: buckets
    });
  `);

  assert.equal(behaviour.plain.total, 42);
  assert.deepEqual(behaviour.plain.values, [
    { value: "red", label: "Красный", count: 3 },
  ]);
  assert.equal(behaviour.plain.isTruncated, false);
  // More buckets than the shared cap are cut and reported, not silently dropped.
  assert.equal(behaviour.wide.values.length, 20);
  assert.equal(behaviour.wide.isTruncated, true);
  // An owner that already cut its own list stays truncated even when the list fits.
  assert.equal(behaviour.ownerCut.values.length, 1);
  assert.equal(behaviour.ownerCut.isTruncated, true);
  // Unbounded domains stay non-enumerable with no buckets.
  assert.deepEqual(behaviour.open.values, []);
  assert.equal(behaviour.open.isEnumerable, false);
  assert.equal(behaviour.boolean.domain.kind, "boolean");
  assert.equal(behaviour.boolean.isEnumerable, true);
  assert.deepEqual(behaviour.boolean.values.map((bucket) => bucket.value), ["true", "false"]);
  assert.deepEqual(behaviour.multi.domain, { kind: "dictionary", multi: true });
  assert.deepEqual(behaviour.codes, ["color", "size"]);
  assert.deepEqual(behaviour.noOptions, []);
  // The product mapper and the shared toolkit answer identically for the same buckets.
  assert.deepEqual(behaviour.shared.values, behaviour.wide.values);
  assert.equal(behaviour.shared.isTruncated, behaviour.wide.isTruncated);
  assert.equal(behaviour.shared.total, behaviour.wide.total);
});

test("the storefront panel renders the owner's panel instead of inventing buckets", () => {
  const nextFilters = read("apps/next-frontend/packages/rustok-product/src/components/product-filters.tsx");
  assert.match(nextFilters, /buildCatalogFacetFiltersView\(/);
  const nextFacets = read(NEXT_FACETS);
  assert.match(nextFacets, /\(facets \?\? \[\]\)\.map\(catalogFacetToSource\)/);
  // The storefront package speaks one scope: its own.
  assert.ok(!nextFacets.includes("adminProductCatalogFacets"));
  assert.ok(!nextFacets.includes("product_attribute_values"));
  assert.match(nextFilters, /facets\?: ProductCatalogFacet\[\];/);
});

test("the owner resolves one selection per attribute code on every read path", () => {
  for (const relative of [OWNER_STOREFRONT_LIST, OWNER_ADMIN_LIST, OWNER_FACETS]) {
    const source = readFileSync(path.join(repoRoot, ...relative.split("/")), "utf8");
    assert.match(source, /load_catalog_attribute_filter_conditions\(/, relative);
  }
  const limits = readFileSync(path.join(repoRoot, ...OWNER_FILTER_LIMITS.split("/")), "utf8");
  // The entry bound is derived from the two caps the gate already pins (8 attributes x 20 values).
  assert.match(
    limits,
    /pub\(crate\) const MAX_ATTRIBUTE_FILTER_ENTRIES: usize =\s*MAX_ATTRIBUTE_FILTERS \* super::facets::MAX_CATALOG_FACET_VALUES;/,
  );
  // The grouping lives in one place, and a repeated code is not an error any more.
  assert.match(limits, /pub fn group_product_attribute_filters\(/);
  assert.doesNotMatch(limits, /occurs more than once/);
  const attributeFilters = readFileSync(
    path.join(repoRoot, ...OWNER_ATTRIBUTE_FILTERS.split("/")),
    "utf8",
  );
  assert.match(attributeFilters, /let mut attribute_condition = Condition::any\(\);/);
  assert.doesNotMatch(attributeFilters, /conditions\.push\(build_attribute_filter_condition\(/);
});

test("the Next panel keeps several values of one attribute in one selection", () => {
  const behaviour = runNextFacets(`
checks.toggled = facets.toggleAttributeFilter(["color=red"], "color", "blue");
checks.cleared = facets.clearAttributeFilterCode(checks.toggled, "color");
checks.widened = facets.toggleAttributeFilter(checks.toggled, "size", "m");
`);
  // The owner accepts this vocabulary: two values of one attribute are one OR selection, and the
  // panel must not silently replace the previous value of the same attribute.
  assert.deepEqual(behaviour.toggled, ["color=red", "color=blue"]);
  assert.deepEqual(behaviour.cleared, []);
  assert.deepEqual(behaviour.widened, ["color=red", "color=blue", "size=m"]);
});
