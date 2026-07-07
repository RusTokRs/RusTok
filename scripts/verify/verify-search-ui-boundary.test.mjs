#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-search-ui-boundary.mjs");

function writeFixtureFile(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function adminCore({ leptos = false, omitPreviewRequest = false } = {}) {
  return `
${leptos ? "use leptos::prelude::*;" : ""}
${omitPreviewRequest ? "" : "pub fn build_search_preview_request() {}"}
pub fn build_search_preview_view_model() {}
pub fn build_search_analytics_summary_view_model() {}
pub fn build_lagging_search_document_row_view_models() {}
pub fn build_search_consistency_issue_row_view_models() {}
pub fn build_search_synonym_mutation_request() {}
pub fn build_search_pin_rule_mutation_request() {}
`;
}

function storefrontCore({ omitRouteIntent = false } = {}) {
  return `
pub fn facet_bucket_display_label() {}
pub fn build_search_results_view_model() {}
pub fn build_search_suggestion_view_models() {}
pub fn build_search_preset_chip_view_models() {}
pub fn build_search_facet_view_models() {}
pub fn build_search_result_action_view_model() {}
pub fn build_storefront_search_fetch_request() {}
${omitRouteIntent ? "" : "pub fn build_storefront_search_route_intent() {}"}
pub fn build_storefront_suggestion_fetch_request() {}
fn route(route_filters: RouteFilters) {
  let _ = Filters {
    channel_id: route_filters.channel_id,
    category_ids: route_filters.category_ids,
    attribute_filters: route_filters.attribute_filters,
    attribute_code: optional_text(attribute_code.unwrap_or_default()),
    attribute_values: route_filters.attribute_values,
    values: route_filters.attribute_values,
    sort_attribute_code: route_filters.sort_attribute_code,
    sort_desc: route_filters.sort_desc,
  };
}
pub struct SearchRouteFilters {
  pub attribute_code: Option<String>,
  pub attribute_values: Vec<String>,
}
`;
}

function uiModel() {
  return `
pub struct SearchFacetBucket {
  pub value: String,
  pub label: Option<String>,
  pub count: u64,
}
pub struct SearchAttributeFilter {
  pub attribute_code: String,
}
pub struct SearchPreviewFilters {
  pub channel_id: Option<String>,
  pub category_ids: Vec<String>,
  pub attribute_filters: Vec<SearchAttributeFilter>,
  pub sort_attribute_code: Option<String>,
  pub sort_desc: bool,
}
`;
}

function adminUi({ rawApi = false } = {}) {
  return `
use crate::{core, transport};
pub fn render() {
  let _ = core::build_search_preview_request;
  let _ = transport::fetch_search_preview;
  let _ = transport::fetch_dictionary_snapshot;
  ${rawApi ? "let _ = api::fetch_search_preview;" : ""}
}
`;
}

function storefrontUi({ rawAdapter = false } = {}) {
  return `
use crate::{core, transport};
pub fn render() {
  let _ = core::build_storefront_search_fetch_request;
  let _ = core::build_storefront_suggestion_fetch_request;
  let _ = transport::fetch_search;
  let _ = transport::track_search_click;
  ${rawAdapter ? "let _ = native_server_adapter::fetch_search;" : ""}
}
`;
}

function searchEngine({ omitCatalogQuery = false } = {}) {
  return `
pub struct SearchQuery {
  pub channel_id: Option<Uuid>,
  ${omitCatalogQuery ? "" : "pub category_ids: Vec<Uuid>,"}
  pub attribute_filters: Vec<SearchAttributeFilter>,
  pub sort_attribute_code: Option<String>,
  pub sort_desc: bool,
}
pub struct SearchAttributeFilter {}
pub struct SearchFacetBucket {
  pub label: Option<String>,
}
`;
}

function pgEngine({ omitProjectionSql = false } = {}) {
  return `
fn query() {
  let _ = "index_product_categories";
  ${omitProjectionSql ? "" : 'let _ = "index_product_attribute_values";'}
  let _ = channel_scope_clause;
  let _ = "iav.channel_id IS NULL";
  let _ = "iav.is_filterable = TRUE";
  let _ = "iav.is_detached = FALSE";
  let _ = "('attr:' || iav.attribute_code)";
  let _ = "facet_label";
  let _ = "value_number";
  let _ = normalized_sort_attribute_code;
}
`;
}

function dictionaries() {
  return `
fn apply(query: Query) {
  if has_catalog_filters(query) {
    continue;
  }
}
`;
}

function graphqlTypes() {
  return `
pub struct SearchPreviewInput {
  pub channel_id: Option<String>,
  pub category_ids: Option<Vec<String>>,
  pub attribute_filters: Option<Vec<SearchAttributeFilterInput>>,
  pub sort_attribute_code: Option<String>,
  pub sort_desc: Option<bool>,
}
pub struct SearchAttributeFilterInput {}
pub struct SearchFacetBucketPayload {
  pub label: Option<String>,
}
`;
}

function graphqlQuery() {
  return `
struct NormalizedSearchPreviewInput {}
fn normalize(input: Input) {
  let _ = parse_optional_uuid(input.channel_id.as_deref());
  let _ = normalize_uuid_values("category_ids", input.category_ids);
  let _ = normalize_attribute_filters(input.attribute_filters);
  let _ = normalize_attribute_code(input.sort_attribute_code);
  let _ = Query {
    category_ids: input.category_ids,
    attribute_filters: input.attribute_filters,
    sort_attribute_code: input.sort_attribute_code,
    sort_desc: input.sort_desc,
  };
}
`;
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-search-ui-boundary-"));
  writeFixtureFile(root, "crates/rustok-search/admin/src/lib.rs", `${options.legacyAdminModApi ? "mod api;\n" : ""}mod core;\nmod transport;\nmod ui;\npub use ui::leptos::SearchAdmin;\npub use ui::leptos::SearchCatalogFilterOption;\n`);
  writeFixtureFile(root, "crates/rustok-search/admin/src/model.rs", uiModel());
  writeFixtureFile(root, "crates/rustok-search/admin/src/core.rs", `${adminCore(options)}
pub struct SearchPreviewFormInput<'a> {
  pub channel_id: &'a str,
  pub category_ids: &'a str,
  pub attribute_code: &'a str,
  pub attribute_values: &'a str,
  pub sort_attribute_code: &'a str,
}
fn build(input: SearchPreviewFormInput) {
  let _ = optional_text(input.attribute_code);
  let _ = category_ids: parse_csv(input.category_ids);
  let _ = sort_attribute_code: optional_text(input.sort_attribute_code);
}
`);
  writeFixtureFile(root, "crates/rustok-search/admin/src/ui/leptos.rs", `${adminUi(options)}
fn catalog_controls() {
  pub struct SearchCatalogFilterOption {}
  let _ = category_options: Vec<SearchCatalogFilterOption>;
  let _ = attribute_options: Vec<SearchCatalogFilterOption>;
  let _ = "search.playground.channelId";
  let _ = "search.playground.categoryIds";
  let _ = "search.playground.attributeCode";
  let _ = "search.playground.attributeValues";
  let _ = "search.playground.attributeMin";
  let _ = "search.playground.attributeMax";
  let _ = "search.playground.sortAttribute";
  let _ = "search.playground.sortDesc";
  let _ = set_channel_id;
  let _ = set_category_ids;
  let _ = set_attribute_code;
  let _ = set_sort_attribute_code;
  let _ = CatalogFilterOptions;
  let _ = "search-admin-category-options";
  let _ = "search-admin-sort-attribute-options";
}
`);
  writeFixtureFile(root, "crates/rustok-search/admin/src/transport/mod.rs", "mod native_server_adapter;\npub type TransportError = native_server_adapter::ApiError;\npub async fn fetch_bootstrap() {}\npub async fn fetch_search_preview() { let _ = native_server_adapter::fetch_search_preview; }\npub async fn fetch_search_analytics() {}\npub async fn fetch_dictionary_snapshot() {}\npub async fn update_search_settings() {}\n");
  writeFixtureFile(root, "crates/rustok-search/admin/src/transport/native_server_adapter.rs", "use rustok_graphql as graphql;\npub enum ApiError {}\npub struct SearchAttributeFilterInput {}\n#[server]\npub async fn endpoint() {}\npub fn fetch_search_preview() { let _ = \"value label count\"; let _ = channel_id: filters.channel_id; let _ = parse_optional_uuid(input.channel_id.as_deref()); let _ = normalize_uuid_values(\"category_ids\", input.category_ids); let _ = normalize_attribute_filters(input.attribute_filters); let _ = normalize_attribute_code(input.sort_attribute_code); let _ = sort_desc: input.sort_desc.unwrap_or(false); let _ = label: bucket.label; }\n");
  if (options.legacyAdminApi) {
    writeFixtureFile(root, "crates/rustok-search/admin/src/api.rs", "pub async fn fetch_search_preview() {}\n");
  }

  writeFixtureFile(root, "crates/rustok-search/storefront/src/lib.rs", "mod core;\nmod transport;\nmod ui;\npub use ui::leptos::SearchView;\npub use ui::leptos::SearchCatalogFilterOption;\n");
  writeFixtureFile(root, "crates/rustok-search/storefront/src/model.rs", uiModel());
  writeFixtureFile(root, "crates/rustok-search/storefront/src/core.rs", storefrontCore(options));
  writeFixtureFile(root, "crates/rustok-search/storefront/src/ui/leptos.rs", `${storefrontUi(options)}
fn catalog_route_controls() {
  pub struct SearchCatalogFilterOption {}
  let _ = category_options: Vec<SearchCatalogFilterOption>;
  let _ = attribute_options: Vec<SearchCatalogFilterOption>;
  let _ = read_route_query_value(&route_context, "channel_id");
  let _ = read_route_query_value(&route_context, "category_ids");
  let _ = read_route_query_value(&route_context, "attribute_code");
  let _ = read_route_query_value(&route_context, "attribute_values");
  let _ = read_route_query_value(&route_context, "sort_attribute_code");
  let _ = "search.filters.title";
  let _ = "search.filters.attributeCode";
  let _ = CatalogFilterField;
  let _ = datalist id=list_id;
  let _ = "search-storefront-category-options";
  let _ = "search-storefront-sort-attribute-options";
  let _ = navigate_to_catalog_search;
  let _ = ("attribute_code", attribute_code);
  let _ = ("sort_attribute_code", sort_attribute_code);
}
`);
  writeFixtureFile(root, "crates/rustok-search/storefront/src/transport/mod.rs", "pub mod graphql_adapter;\npub mod native_server_adapter;\npub async fn fetch_search() { let _ = native_server_adapter::fetch_search; let _ = graphql_adapter::fetch_search; }\npub async fn fetch_suggestions() { let _ = native_server_adapter::fetch_suggestions; let _ = graphql_adapter::fetch_suggestions; }\n");
  writeFixtureFile(root, "crates/rustok-search/storefront/src/transport/native_server_adapter.rs", "pub fn fetch_storefront_search_server() {}\npub fn fetch_storefront_suggestions_server() {}\npub fn fetch_search() {}\npub fn fetch_suggestions() {}\n");
  writeFixtureFile(root, "crates/rustok-search/storefront/src/transport/graphql_adapter.rs", "use rustok_graphql::GraphqlRequest;\npub struct SearchAttributeFilterInput {}\npub fn fetch_storefront_search_graphql() { let _ = \"value label count\"; let _ = channel_id: filters.channel_id; let _ = category_ids: (!filters.category_ids.is_empty()).then_some(filters.category_ids); let _ = attribute_filters: (!filters.attribute_filters.is_empty()); let _ = sort_attribute_code: filters.sort_attribute_code; let _ = sort_desc: filters.sort_desc.then_some(true); }\npub fn fetch_storefront_suggestions_graphql() {}\npub fn fetch_search() {}\npub fn fetch_suggestions() {}\n");
  writeFixtureFile(root, "crates/rustok-search/src/engine.rs", searchEngine(options));
  writeFixtureFile(root, "crates/rustok-search/src/pg_engine.rs", pgEngine(options));
  writeFixtureFile(root, "crates/rustok-search/src/dictionaries.rs", dictionaries());
  writeFixtureFile(root, "crates/rustok-search/src/graphql/types.rs", graphqlTypes());
  writeFixtureFile(root, "crates/rustok-search/src/graphql/query.rs", graphqlQuery());
  writeFixtureFile(root, "apps/next-admin/packages/search/src/index.tsx", `
export type SearchCatalogFilterOption = { value: string; label: string };
export type SearchAdminPageProps = {
  categoryOptions?: SearchCatalogFilterOption[];
  attributeOptions?: SearchCatalogFilterOption[];
};
type SearchPreviewFiltersInput = {
  channelId?: string;
  categoryIds: string[];
  attributeFilters: Array<{ attributeCode: string }>;
  sortAttributeCode?: string;
};
const query = "facets { name buckets { value label count } }";
const input = {
  channelId: optionalText(channelId),
  categoryIds: parseCsv(categoryIds),
  sortAttributeCode: optionalText(sortAttributeCode),
};
function CatalogOptionDatalist() {
  return <datalist id="search-admin-category-options"><option value="category">Category</option></datalist>;
}
const attributeList = "search-admin-attribute-options";
const label = bucket.label || bucket.value;
`);
  writeFixtureFile(root, "apps/next-frontend/packages/search/src/index.tsx", `
export type SearchCatalogFilterOption = { value: string; label: string };
export type SearchCatalogFilters = {};
export type SearchStorefrontPageProps = {
  initialFilters?: Partial<SearchCatalogFilters>;
  categoryOptions?: SearchCatalogFilterOption[];
  attributeOptions?: SearchCatalogFilterOption[];
};
const query = \`facets { buckets { value label count } }\`;
const input = {
  channelId: filters.channelId.trim() || undefined,
  categoryIds: categoryIds.length ? categoryIds : undefined,
  attributeFilters: filters.attributeCode.trim(),
  sortAttributeCode: filters.sortAttributeCode.trim() || undefined,
};
const label = bucket.label || bucket.value;
function CatalogField(props: { options?: SearchCatalogFilterOption[] }) {
  const listId = React.useId();
  return <datalist id={listId}></datalist>;
}
`);
  writeFixtureFile(root, "apps/next-admin/packages/rustok-product/src/index.ts", `
export type ProductCatalogSearchOption = { value: string; label: string };
const ATTRIBUTE_QUERY = "productAttributes(tenantId: $tenantId, locale: $locale)";
const CATEGORY_QUERY = "catalogCategories(tenantId: $tenantId, locale: $locale)";
export async function listCatalogCategorySearchOptions() {}
export async function listCatalogAttributeSearchOptions() {
  return items.filter((attribute) => attribute.isFilterable || attribute.isSortable);
}
`);
  writeFixtureFile(root, "apps/next-frontend/packages/rustok-product/src/index.ts", `
export type ProductCatalogSearchOption = { value: string; label: string };
export type ProductCatalogSearchOptions = {
  categoryOptions: ProductCatalogSearchOption[];
  attributeOptions: ProductCatalogSearchOption[];
};
const QUERY = "query StorefrontCatalogSearchOptions($locale: String!) { storefrontCatalogSearchOptions(locale: $locale) { categoryOptions { value label } attributeOptions { value label } } }";
export async function fetchCatalogSearchOptions(request) {
  const locale = request.locale.trim();
  return storefrontGraphql({ query: QUERY, variables: { locale }, tenant: request.tenantSlug });
}
`);
  writeFixtureFile(root, "apps/next-admin/src/app/dashboard/search/page.tsx", `
import { getLocale } from 'next-intl/server';
import { listCatalogAttributeSearchOptions, listCatalogCategorySearchOptions } from '../../../../packages/rustok-product/src';
async function loadCatalogSearchOptions() {
  const locale = await getLocale();
  const categoryOptions = await listCatalogCategorySearchOptions({ locale });
  const attributeOptions = await listCatalogAttributeSearchOptions({ locale });
  return { categoryOptions, attributeOptions };
}
function Page() {
  return <SearchAdminPage categoryOptions={categoryOptions} attributeOptions={attributeOptions} />;
}
`);
  writeFixtureFile(root, "apps/next-frontend/src/features/search/components/search-section.tsx", options.omitNextStorefrontHostComposition ? "" : `
import SearchStorefrontPage, { type SearchCatalogFilterOption } from "../../../../packages/search/src";
import { fetchCatalogSearchOptions } from "../../../../packages/rustok-product/src";
export function SearchSection({ locale, enabledModules, tenantSlug }) {
  const productEnabled = enabledModules.includes("product");
  if (locale.trim()) {
    fetchCatalogSearchOptions({ locale, tenantSlug });
  }
  return <SearchStorefrontPage categoryOptions={categoryOptions} attributeOptions={attributeOptions} tenantSlug={tenantSlug} />;
}
`);
  writeFixtureFile(root, "apps/next-frontend/src/modules/index.ts", `
import "./core";
import "@/features/blog";
import "@/features/search";
export * from "./registry";
`);
  writeFixtureFile(root, "apps/next-frontend/src/app/[locale]/page.tsx", `
import { fetchEnabledModules, getStorefrontTenantSlug } from "@/shared/api/modules";
async function Page({ params }) {
  const { locale } = await params;
  const tenantSlug = getStorefrontTenantSlug();
  const enabledModules = await fetchEnabledModules(tenantSlug);
  return module.render({ locale, enabledModules, tenantSlug });
}
`);
  writeFixtureFile(root, "crates/rustok-product/admin/src/model.rs", `
pub struct ProductCatalogSearchOption {
  pub value: String,
  pub label: String,
}
pub struct ProductCatalogSearchOptions {
  pub category_options: Vec<ProductCatalogSearchOption>,
  pub attribute_options: Vec<ProductCatalogSearchOption>,
}
`);
  writeFixtureFile(root, "crates/rustok-product/admin/src/transport.rs", `
use crate::model::{ProductCatalogSearchOption, ProductCatalogSearchOptions};
${options.omitProductLeptosMetadata ? "" : `pub async fn fetch_catalog_search_options() {
  if let Ok(options) = native_server_adapter::fetch_catalog_search_options(locale.clone()) { return Ok(options); }
  let bootstrap = graphql_adapter::fetch_bootstrap(token.clone(), tenant_slug.clone());
  let tenant_id = bootstrap.current_tenant.id;
  let categories = graphql_adapter::fetch_catalog_categories(token.clone(), tenant_slug.clone(), tenant_id.clone(), locale.clone());
  let attributes = graphql_adapter::fetch_product_attributes(token, tenant_slug, tenant_id, locale);
  let _ = items.filter(|attribute| attribute.is_filterable || attribute.is_sortable);
}`}
`);
  writeFixtureFile(root, "crates/rustok-product/admin/src/transport/native_server_adapter.rs", `
pub async fn fetch_catalog_search_options(locale: String) {
  product_admin_catalog_search_options_native(locale);
}
#[server(prefix = "/api/fn", endpoint = "product/admin/catalog-search-options")]
async fn product_admin_catalog_search_options_native(locale: String) {
  let (_, auth, tenant) = native_context().await?;
  let _ = Permission::PRODUCTS_READ;
  let _ = list_categories(tenant.id, locale.trim());
  let _ = list_attributes(tenant.id, locale.trim());
  let _ = items.filter(|attribute| attribute.is_filterable || attribute.is_sortable);
}
`);
  writeFixtureFile(root, "crates/rustok-product/admin/src/lib.rs", `
pub use model::{ProductCatalogSearchOption, ProductCatalogSearchOptions};
pub use transport::fetch_catalog_search_options;
`);
  writeFixtureFile(root, "apps/admin/build.rs", `
fn render(entry: Entry) {
  let page_component_path = if entry.slug == "search" {
    "crate::app::modules::SearchAdminComposition"
  } else {
    "owner"
  };
}
`);
  writeFixtureFile(root, "apps/admin/src/app/modules/search_composition.rs", options.omitLeptosHostComposition ? "" : `
use rustok_api::UiRouteContext;
use rustok_product_admin::fetch_catalog_search_options;
use rustok_search_admin::{SearchAdmin, SearchCatalogFilterOption};
fn compose() {
  let product_enabled = use_is_module_enabled("product");
  let token = use_token();
  let tenant = use_tenant();
  if locale.trim().is_empty() { return; }
  let _ = category_options=category_options;
  let _ = attribute_options=attribute_options;
}
`);
  writeFixtureFile(root, "crates/rustok-product/storefront/src/model.rs", `
pub struct ProductCatalogSearchOption { pub value: String, pub label: String }
pub struct ProductCatalogSearchOptions {
  pub category_options: Vec<ProductCatalogSearchOption>,
  pub attribute_options: Vec<ProductCatalogSearchOption>,
}
`);
  writeFixtureFile(root, "crates/rustok-product/storefront/src/transport/mod.rs", `
pub async fn fetch_catalog_search_options(locale: String) {
  match native_server_adapter::fetch_catalog_search_options(locale.clone()).await {
    Ok(data) => Ok(data),
    Err(native_error) => match graphql_adapter::fetch_catalog_search_options(locale).await {
      Err(graphql_error) => Err(ProductTransportError::fallback_failed(native_error, graphql_error)),
    }
  }
}
`);
  writeFixtureFile(root, "crates/rustok-product/storefront/src/transport/native_server_adapter.rs", `
pub async fn fetch_catalog_search_options(locale: String) {
  storefront_catalog_search_options_native(locale);
}
#[server(prefix = "/api/fn", endpoint = "product/storefront/catalog-search-options")]
async fn storefront_catalog_search_options_native(locale: String) {
  if locale.trim().is_empty() { return Err("locale is required"); }
  let tenant = extract::<TenantContext>();
  let service = ProductCatalogSchemaService;
  let _ = list_categories(tenant.id, locale.trim());
  let _ = list_attributes(tenant.id, locale.trim());
  let _ = items.filter(|attribute| attribute.is_filterable || attribute.is_sortable);
}
`);
  writeFixtureFile(root, "crates/rustok-product/storefront/src/transport/graphql_adapter.rs", `
const QUERY: &str = "query StorefrontCatalogSearchOptions($locale: String!) { storefrontCatalogSearchOptions(locale: $locale) { categoryOptions { value label } attributeOptions { value label } } }";
pub async fn fetch_catalog_search_options() {}
`);
  writeFixtureFile(root, "crates/rustok-product/storefront/src/lib.rs", `
pub use model::{ProductCatalogSearchOption, ProductCatalogSearchOptions};
pub use transport::fetch_catalog_search_options;
`);
  writeFixtureFile(root, "crates/rustok-commerce/src/graphql/query.rs", `
async fn storefront_catalog_search_options(ctx: Context, locale: String) {
  require_module_enabled(ctx, "product");
  require_storefront_channel_enabled(ctx);
  if locale.trim().is_empty() { return Err("locale is required"); }
  let tenant = ctx.data::<TenantContext>();
  let _ = list_categories(tenant.id, locale.trim());
  let _ = list_attributes(tenant.id, locale.trim());
  let _ = items.filter(|attribute| attribute.is_filterable || attribute.is_sortable);
}
`);
  writeFixtureFile(root, "crates/rustok-commerce/src/graphql/types.rs", `
pub struct GqlProductCatalogSearchOption {}
pub struct GqlProductCatalogSearchOptions {}
`);
  writeFixtureFile(root, "apps/storefront/build.rs", `
fn render(entry: Entry) {
  let component_path = if entry.slug == "search" {
    "crate::modules::SearchStorefrontComposition"
  } else {
    "owner"
  };
}
`);
  writeFixtureFile(root, "apps/storefront/src/modules/search_composition.rs", options.omitStorefrontHostComposition ? "" : `
use rustok_api::UiRouteContext;
use rustok_product_storefront::fetch_catalog_search_options;
use rustok_search_storefront::{SearchCatalogFilterOption, SearchView};
fn compose() {
  let product_enabled = use_is_module_enabled("product");
  if locale.trim().is_empty() { return; }
  let _ = category_options=category_options;
  let _ = attribute_options=attribute_options;
}
`);
  return root;
}

function runVerifier(root) {
  return spawnSync("node", [scriptPath], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

test("search UI boundary verifier passes canonical fixture", () => {
  const root = withFixture();
  try {
    const result = runVerifier(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /search UI boundary verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("search UI boundary verifier rejects Leptos-specific admin core", () => {
  const root = withFixture({ leptos: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /core must stay Leptos\/server-function free/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("search UI boundary verifier rejects missing admin preview request policy", () => {
  const root = withFixture({ omitPreviewRequest: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /build_search_preview_request/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("search UI boundary verifier rejects raw admin api calls from UI", () => {
  const root = withFixture({ rawApi: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /UI adapter must not call raw transport/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("search UI boundary verifier rejects legacy admin api file", () => {
  const root = withFixture({ legacyAdminApi: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /admin legacy api\.rs/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("search UI boundary verifier rejects legacy admin api module", () => {
  const root = withFixture({ legacyAdminModApi: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must not wire legacy api module/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("search UI boundary verifier rejects missing storefront route intent policy", () => {
  const root = withFixture({ omitRouteIntent: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /build_storefront_search_route_intent/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("search UI boundary verifier rejects raw storefront adapter calls from UI", () => {
  const root = withFixture({ rawAdapter: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /UI adapter must not call raw transport/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("search UI boundary verifier rejects missing catalog query markers", () => {
  const root = withFixture({ omitCatalogQuery: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /catalog projection query\/result contract/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("search UI boundary verifier rejects missing projection SQL markers", () => {
  const root = withFixture({ omitProjectionSql: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /PostgreSQL catalog projection search marker/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("search UI boundary verifier rejects missing product Leptos metadata helper", () => {
  const root = withFixture({ omitProductLeptosMetadata: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /product Leptos metadata helper marker missing/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("search UI boundary verifier rejects missing Leptos host catalog composition", () => {
  const root = withFixture({ omitLeptosHostComposition: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /Leptos host catalog composition marker missing/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("search UI boundary verifier rejects missing storefront host catalog composition", () => {
  const root = withFixture({ omitStorefrontHostComposition: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /storefront host catalog composition marker missing/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("search UI boundary verifier rejects missing Next storefront host catalog composition", () => {
  const root = withFixture({ omitNextStorefrontHostComposition: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /Next storefront host catalog composition marker missing/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
