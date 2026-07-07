#!/usr/bin/env node
// RusTok search FFA boundary guardrails.
// Fast source-level checks for the module-owned admin/storefront core/transport/ui split.

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
  return readFileSync(repoPath(relativePath), "utf8");
}

function fail(message) {
  failures.push(message);
}

function assertExists(relativePath, description) {
  if (!existsSync(repoPath(relativePath))) fail(description);
}

function assertContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) fail(description);
}

function assertNotContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (found) fail(description);
}

function assertSearchAdminBoundary() {
  const libPath = "crates/rustok-search/admin/src/lib.rs";
  const corePath = "crates/rustok-search/admin/src/core.rs";
  const uiPath = "crates/rustok-search/admin/src/ui/leptos.rs";
  const transportPath = "crates/rustok-search/admin/src/transport/mod.rs";
  const nativePath = "crates/rustok-search/admin/src/transport/native_server_adapter.rs";
  const legacyApiPath = "crates/rustok-search/admin/src/api.rs";

  for (const checkedPath of [libPath, corePath, uiPath, transportPath, nativePath]) {
    assertExists(checkedPath, `${checkedPath}: expected search admin boundary file`);
  }
  if (existsSync(repoPath(legacyApiPath))) {
    fail(`${legacyApiPath}: admin legacy api.rs must stay removed; transport adapters own native/GraphQL paths`);
  }

  const lib = readRepo(libPath);
  const core = readRepo(corePath);
  const ui = readRepo(uiPath);
  const transport = readRepo(transportPath);
  const native = readRepo(nativePath);

  assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
  assertNotContains(lib, "mod api;", `${libPath}: crate root must not wire legacy api module`);
  assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
  assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
  assertContains(lib, "pub use ui::leptos::SearchAdmin;", `${libPath}: crate root must re-export SearchAdmin`);

  for (const marker of ["leptos::", "leptos_", "web_sys", "#[component]", "#[server", "LocalResource"]) {
    assertNotContains(core, marker, `${corePath}: core must stay Leptos/server-function free (${marker})`);
  }
  for (const marker of [
    "build_search_preview_request",
    "build_search_preview_view_model",
    "build_search_analytics_summary_view_model",
    "build_lagging_search_document_row_view_models",
    "build_search_consistency_issue_row_view_models",
    "build_search_synonym_mutation_request",
    "build_search_pin_rule_mutation_request",
  ]) {
    assertContains(core, marker, `${corePath}: core must own ${marker} policy`);
  }

  assertContains(ui, "use crate::{core, transport};", `${uiPath}: UI adapter must import the module transport facade`);
  assertContains(ui, "core::build_search_preview_request", `${uiPath}: UI adapter must consume core-owned preview request policy`);
  assertContains(ui, "transport::fetch_search_preview", `${uiPath}: UI adapter must call transport facade for preview`);
  assertContains(ui, "transport::fetch_dictionary_snapshot", `${uiPath}: UI adapter must call transport facade for dictionaries`);
  for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "rustok_graphql::", "execute_graphql", "#[server"] ) {
    assertNotContains(ui, marker, `${uiPath}: UI adapter must not call raw transport (${marker})`);
  }

  assertContains(transport, "mod native_server_adapter;", `${transportPath}: admin transport facade must wire native server adapter`);
  assertContains(transport, "pub type TransportError = native_server_adapter::ApiError;", `${transportPath}: admin transport facade must own exposed transport error alias`);
  for (const marker of ["fetch_bootstrap", "fetch_search_preview", "fetch_search_analytics", "fetch_dictionary_snapshot", "update_search_settings"]) {
    assertContains(transport, `pub async fn ${marker}`, `${transportPath}: facade must expose ${marker}`);
  }
  assertContains(transport, "native_server_adapter::fetch_search_preview", `${transportPath}: facade must delegate preview through native adapter`);
  assertNotContains(transport, "#[server", `${transportPath}: server functions belong in native_server_adapter.rs`);
  assertNotContains(transport, "crate::api", `${transportPath}: transport facade must not delegate to legacy api module`);

  assertContains(native, "#[server", `${nativePath}: admin raw adapter must keep native server-function endpoints`);
  assertContains(native, "rustok_graphql", `${nativePath}: admin raw adapter must keep GraphQL fallback implementation`);
}

function assertSearchStorefrontBoundary() {
  const libPath = "crates/rustok-search/storefront/src/lib.rs";
  const corePath = "crates/rustok-search/storefront/src/core.rs";
  const uiPath = "crates/rustok-search/storefront/src/ui/leptos.rs";
  const transportPath = "crates/rustok-search/storefront/src/transport/mod.rs";
  const nativePath = "crates/rustok-search/storefront/src/transport/native_server_adapter.rs";
  const graphqlPath = "crates/rustok-search/storefront/src/transport/graphql_adapter.rs";
  const legacyApiPath = "crates/rustok-search/storefront/src/api.rs";

  for (const checkedPath of [libPath, corePath, uiPath, transportPath, nativePath, graphqlPath]) {
    assertExists(checkedPath, `${checkedPath}: expected search storefront boundary file`);
  }
  if (existsSync(repoPath(legacyApiPath))) {
    fail(`${legacyApiPath}: storefront legacy api.rs must stay removed; transport adapters own native/GraphQL paths`);
  }

  const lib = readRepo(libPath);
  const core = readRepo(corePath);
  const ui = readRepo(uiPath);
  const transport = readRepo(transportPath);
  const native = readRepo(nativePath);
  const graphql = readRepo(graphqlPath);

  assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
  assertNotContains(lib, "mod api;", `${libPath}: crate root must not wire legacy api module`);
  assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
  assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
  assertContains(lib, "pub use ui::leptos::SearchView;", `${libPath}: crate root must re-export SearchView`);

  for (const marker of ["leptos::", "leptos_", "web_sys", "#[component]", "#[server", "LocalResource"]) {
    assertNotContains(core, marker, `${corePath}: core must stay Leptos/server-function free (${marker})`);
  }
  for (const marker of [
    "build_search_results_view_model",
    "build_search_suggestion_view_models",
    "build_search_preset_chip_view_models",
    "build_search_facet_view_models",
    "build_search_result_action_view_model",
    "build_storefront_search_fetch_request",
    "build_storefront_search_route_intent",
    "build_storefront_suggestion_fetch_request",
  ]) {
    assertContains(core, marker, `${corePath}: core must own ${marker} policy`);
  }

  assertContains(ui, "use crate::{core, transport};", `${uiPath}: UI adapter must import the module transport facade`);
  assertContains(ui, "core::build_storefront_search_fetch_request", `${uiPath}: UI adapter must consume core-owned search fetch policy`);
  assertContains(ui, "core::build_storefront_suggestion_fetch_request", `${uiPath}: UI adapter must consume core-owned suggestion fetch policy`);
  assertContains(ui, "transport::fetch_search", `${uiPath}: UI adapter must call transport facade for search`);
  assertContains(ui, "transport::track_search_click", `${uiPath}: UI adapter must call transport facade for click tracking`);
  for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "native_server_adapter::", "graphql_adapter::", "rustok_graphql::", "execute_graphql", "#[server"] ) {
    assertNotContains(ui, marker, `${uiPath}: UI adapter must not call raw transport (${marker})`);
  }

  assertContains(transport, "pub mod graphql_adapter;", `${transportPath}: transport facade must wire GraphQL fallback adapter`);
  assertContains(transport, "pub mod native_server_adapter;", `${transportPath}: transport facade must wire native server adapter`);
  assertContains(transport, "native_server_adapter::fetch_search", `${transportPath}: facade must prefer native search path`);
  assertContains(transport, "graphql_adapter::fetch_search", `${transportPath}: facade must keep GraphQL search fallback`);
  assertContains(transport, "native_server_adapter::fetch_suggestions", `${transportPath}: facade must prefer native suggestions path`);
  assertContains(transport, "graphql_adapter::fetch_suggestions", `${transportPath}: facade must keep GraphQL suggestions fallback`);
  assertNotContains(transport, "#[server", `${transportPath}: server-function endpoints belong in native_server_adapter.rs`);
  assertNotContains(transport, "crate::api", `${transportPath}: transport facade must not delegate to legacy api module`);
  assertNotContains(transport, "execute_graphql", `${transportPath}: raw GraphQL execution belongs in graphql_adapter.rs`);

  assertContains(native, "fetch_storefront_search_server", `${nativePath}: native adapter must call native search endpoint`);
  assertContains(native, "fetch_storefront_suggestions_server", `${nativePath}: native adapter must call native suggestions endpoint`);
  assertNotContains(native, "fetch_storefront_search_graphql", `${nativePath}: native adapter must not call GraphQL fallback`);

  assertContains(graphql, "fetch_storefront_search_graphql", `${graphqlPath}: GraphQL adapter must call search fallback`);
  assertContains(graphql, "fetch_storefront_suggestions_graphql", `${graphqlPath}: GraphQL adapter must call suggestions fallback`);
  assertNotContains(graphql, "fetch_storefront_search_server", `${graphqlPath}: GraphQL adapter must not call native server path`);
  assertContains(graphql, "GraphqlRequest", `${graphqlPath}: GraphQL adapter must own raw GraphQL request execution`);
}

function assertSearchCatalogProjectionContract() {
  const enginePath = "crates/rustok-search/src/engine.rs";
  const pgEnginePath = "crates/rustok-search/src/pg_engine.rs";
  const dictionariesPath = "crates/rustok-search/src/dictionaries.rs";
  const graphqlTypesPath = "crates/rustok-search/src/graphql/types.rs";
  const graphqlQueryPath = "crates/rustok-search/src/graphql/query.rs";

  for (const checkedPath of [enginePath, pgEnginePath, dictionariesPath, graphqlTypesPath, graphqlQueryPath]) {
    assertExists(checkedPath, `${checkedPath}: expected search catalog projection contract file`);
  }

  const engine = readRepo(enginePath);
  const pgEngine = readRepo(pgEnginePath);
  const dictionaries = readRepo(dictionariesPath);
  const graphqlTypes = readRepo(graphqlTypesPath);
  const graphqlQuery = readRepo(graphqlQueryPath);

  for (const marker of [
    "pub channel_id: Option<Uuid>",
    "pub category_ids: Vec<Uuid>",
    "pub attribute_filters: Vec<SearchAttributeFilter>",
    "pub sort_attribute_code: Option<String>",
    "pub sort_desc: bool",
    "pub struct SearchAttributeFilter",
    "pub label: Option<String>",
  ]) {
    assertContains(engine, marker, `${enginePath}: catalog projection query/result contract missing ${marker}`);
  }

  for (const marker of [
    "index_product_categories",
    "index_product_attribute_values",
    "channel_scope_clause",
    "iav.channel_id IS NULL",
    "iav.is_filterable = TRUE",
    "iav.is_detached = FALSE",
    "('attr:' || iav.attribute_code)",
    "facet_label",
    "value_number",
    "normalized_sort_attribute_code",
  ]) {
    assertContains(pgEngine, marker, `${pgEnginePath}: PostgreSQL catalog projection search marker missing ${marker}`);
  }

  assertContains(dictionaries, "has_catalog_filters(query)", `${dictionariesPath}: pinned query rules must respect catalog filters`);
  assertContains(dictionaries, "continue;", `${dictionariesPath}: pinned query rules must skip raw reload when catalog filters are present`);

  for (const marker of [
    "pub channel_id: Option<String>",
    "pub category_ids: Option<Vec<String>>",
    "pub attribute_filters: Option<Vec<SearchAttributeFilterInput>>",
    "pub sort_attribute_code: Option<String>",
    "pub sort_desc: Option<bool>",
    "pub struct SearchAttributeFilterInput",
    "pub label: Option<String>",
  ]) {
    assertContains(graphqlTypes, marker, `${graphqlTypesPath}: GraphQL catalog projection schema marker missing ${marker}`);
  }

  for (const marker of [
    "NormalizedSearchPreviewInput",
    "parse_optional_uuid(input.channel_id.as_deref())",
    "normalize_uuid_values(\"category_ids\"",
    "normalize_attribute_filters(input.attribute_filters)",
    "normalize_attribute_code(input.sort_attribute_code)",
    "category_ids: input.category_ids",
    "attribute_filters: input.attribute_filters",
    "sort_attribute_code: input.sort_attribute_code",
    "sort_desc: input.sort_desc",
  ]) {
    assertContains(graphqlQuery, marker, `${graphqlQueryPath}: GraphQL catalog projection normalization marker missing ${marker}`);
  }
}

function assertSearchUiCatalogTransportContract() {
  const adminCorePath = "crates/rustok-search/admin/src/core.rs";
  const adminModelPath = "crates/rustok-search/admin/src/model.rs";
  const adminUiPath = "crates/rustok-search/admin/src/ui/leptos.rs";
  const adminNativePath = "crates/rustok-search/admin/src/transport/native_server_adapter.rs";
  const storefrontModelPath = "crates/rustok-search/storefront/src/model.rs";
  const storefrontCorePath = "crates/rustok-search/storefront/src/core.rs";
  const storefrontUiPath = "crates/rustok-search/storefront/src/ui/leptos.rs";
  const storefrontGraphqlPath = "crates/rustok-search/storefront/src/transport/graphql_adapter.rs";
  const nextAdminPath = "apps/next-admin/packages/search/src/index.tsx";
  const nextStorefrontPath = "apps/next-frontend/packages/search/src/index.tsx";
  const nextProductPath = "apps/next-admin/packages/rustok-product/src/index.ts";
  const nextStorefrontProductPath = "apps/next-frontend/packages/rustok-product/src/index.ts";
  const nextSearchPagePath = "apps/next-admin/src/app/dashboard/search/page.tsx";
  const nextStorefrontSearchFeaturePath = "apps/next-frontend/src/features/search/components/search-section.tsx";
  const nextStorefrontModulesPath = "apps/next-frontend/src/modules/index.ts";
  const nextStorefrontHomePath = "apps/next-frontend/src/app/[locale]/page.tsx";
  const productAdminLibPath = "crates/rustok-product/admin/src/lib.rs";
  const productAdminModelPath = "crates/rustok-product/admin/src/model.rs";
  const productAdminTransportPath = "crates/rustok-product/admin/src/transport.rs";
  const productAdminNativePath = "crates/rustok-product/admin/src/transport/native_server_adapter.rs";
  const adminBuildPath = "apps/admin/build.rs";
  const adminCompositionPath = "apps/admin/src/app/modules/search_composition.rs";
  const productStorefrontLibPath = "crates/rustok-product/storefront/src/lib.rs";
  const productStorefrontModelPath = "crates/rustok-product/storefront/src/model.rs";
  const productStorefrontTransportPath = "crates/rustok-product/storefront/src/transport/mod.rs";
  const productStorefrontNativePath = "crates/rustok-product/storefront/src/transport/native_server_adapter.rs";
  const productStorefrontGraphqlPath = "crates/rustok-product/storefront/src/transport/graphql_adapter.rs";
  const searchStorefrontLibPath = "crates/rustok-search/storefront/src/lib.rs";
  const storefrontBuildPath = "apps/storefront/build.rs";
  const storefrontCompositionPath = "apps/storefront/src/modules/search_composition.rs";
  const commerceQueryPath = "crates/rustok-commerce/src/graphql/query.rs";
  const commerceTypesPath = "crates/rustok-commerce/src/graphql/types.rs";

  for (const checkedPath of [
    adminCorePath,
    adminModelPath,
    adminUiPath,
    adminNativePath,
    storefrontModelPath,
    storefrontCorePath,
    storefrontUiPath,
    storefrontGraphqlPath,
    nextAdminPath,
    nextStorefrontPath,
    nextProductPath,
    nextStorefrontProductPath,
    nextSearchPagePath,
    nextStorefrontSearchFeaturePath,
    nextStorefrontModulesPath,
    nextStorefrontHomePath,
    productAdminLibPath,
    productAdminModelPath,
    productAdminTransportPath,
    productAdminNativePath,
    adminBuildPath,
    adminCompositionPath,
    productStorefrontLibPath,
    productStorefrontModelPath,
    productStorefrontTransportPath,
    productStorefrontNativePath,
    productStorefrontGraphqlPath,
    searchStorefrontLibPath,
    storefrontBuildPath,
    storefrontCompositionPath,
    commerceQueryPath,
    commerceTypesPath,
  ]) {
    assertExists(checkedPath, `${checkedPath}: expected search UI catalog transport file`);
  }

  const adminCore = readRepo(adminCorePath);
  const adminModel = readRepo(adminModelPath);
  const adminUi = readRepo(adminUiPath);
  const adminNative = readRepo(adminNativePath);
  const storefrontModel = readRepo(storefrontModelPath);
  const storefrontCore = readRepo(storefrontCorePath);
  const storefrontUi = readRepo(storefrontUiPath);
  const storefrontGraphql = readRepo(storefrontGraphqlPath);
  const nextAdmin = readRepo(nextAdminPath);
  const nextStorefront = readRepo(nextStorefrontPath);
  const nextProduct = readRepo(nextProductPath);
  const nextStorefrontProduct = readRepo(nextStorefrontProductPath);
  const nextSearchPage = readRepo(nextSearchPagePath);
  const nextStorefrontSearchFeature = readRepo(nextStorefrontSearchFeaturePath);
  const nextStorefrontModules = readRepo(nextStorefrontModulesPath);
  const nextStorefrontHome = readRepo(nextStorefrontHomePath);
  const productAdminLib = readRepo(productAdminLibPath);
  const productAdminModel = readRepo(productAdminModelPath);
  const productAdminTransport = readRepo(productAdminTransportPath);
  const productAdminNative = readRepo(productAdminNativePath);
  const adminBuild = readRepo(adminBuildPath);
  const adminComposition = readRepo(adminCompositionPath);
  const productStorefrontLib = readRepo(productStorefrontLibPath);
  const productStorefrontModel = readRepo(productStorefrontModelPath);
  const productStorefrontTransport = readRepo(productStorefrontTransportPath);
  const productStorefrontNative = readRepo(productStorefrontNativePath);
  const productStorefrontGraphql = readRepo(productStorefrontGraphqlPath);
  const searchStorefrontLib = readRepo(searchStorefrontLibPath);
  const storefrontBuild = readRepo(storefrontBuildPath);
  const storefrontComposition = readRepo(storefrontCompositionPath);
  const commerceQuery = readRepo(commerceQueryPath);
  const commerceTypes = readRepo(commerceTypesPath);

  for (const [checkedPath, model] of [
    [adminModelPath, adminModel],
    [storefrontModelPath, storefrontModel],
  ]) {
    for (const marker of [
      "pub label: Option<String>",
      "pub struct SearchAttributeFilter",
      "pub channel_id: Option<String>",
      "pub category_ids: Vec<String>",
      "pub attribute_filters: Vec<SearchAttributeFilter>",
      "pub sort_attribute_code: Option<String>",
      "pub sort_desc: bool",
    ]) {
      assertContains(model, marker, `${checkedPath}: UI DTO catalog transport marker missing ${marker}`);
    }
  }

  for (const marker of [
    "pub channel_id: &'a str",
    "pub category_ids: &'a str",
    "pub attribute_code: &'a str",
    "pub attribute_values: &'a str",
    "pub sort_attribute_code: &'a str",
    "optional_text(input.attribute_code)",
    "category_ids: parse_csv(input.category_ids)",
    "sort_attribute_code: optional_text(input.sort_attribute_code)",
  ]) {
    assertContains(adminCore, marker, `${adminCorePath}: admin core visible catalog form marker missing ${marker}`);
  }

  for (const marker of [
    "pub struct SearchCatalogFilterOption",
    "category_options: Vec<SearchCatalogFilterOption>",
    "attribute_options: Vec<SearchCatalogFilterOption>",
    "search.playground.channelId",
    "search.playground.categoryIds",
    "search.playground.attributeCode",
    "search.playground.attributeValues",
    "search.playground.attributeMin",
    "search.playground.attributeMax",
    "search.playground.sortAttribute",
    "search.playground.sortDesc",
    "set_channel_id",
    "set_category_ids",
    "set_attribute_code",
    "set_sort_attribute_code",
    "CatalogFilterOptions",
    "search-admin-category-options",
    "search-admin-sort-attribute-options",
  ]) {
    assertContains(adminUi, marker, `${adminUiPath}: admin Leptos catalog controls marker missing ${marker}`);
  }
  assertNotContains(adminUi, "rustok_product", `${adminUiPath}: search admin UI must consume host-provided catalog metadata, not product internals`);

  for (const marker of [
    "facet_bucket_display_label",
    "channel_id: route_filters.channel_id",
    "category_ids: route_filters.category_ids",
    "attribute_code: Option<String>",
    "attribute_values: Vec<String>",
    "attribute_code: optional_text(attribute_code.unwrap_or_default())",
    "values: route_filters.attribute_values",
    "attribute_filters",
    "sort_attribute_code: route_filters.sort_attribute_code",
    "sort_desc: route_filters.sort_desc",
  ]) {
    assertContains(storefrontCore, marker, `${storefrontCorePath}: storefront core catalog route marker missing ${marker}`);
  }

  for (const marker of [
    "pub struct SearchCatalogFilterOption",
    "category_options: Vec<SearchCatalogFilterOption>",
    "attribute_options: Vec<SearchCatalogFilterOption>",
    'read_route_query_value(&route_context, "channel_id")',
    'read_route_query_value(&route_context, "category_ids")',
    'read_route_query_value(&route_context, "attribute_code")',
    'read_route_query_value(&route_context, "attribute_values")',
    'read_route_query_value(&route_context, "sort_attribute_code")',
    "search.filters.title",
    "search.filters.attributeCode",
    "CatalogFilterField",
    "datalist id=list_id",
    "search-storefront-category-options",
    "search-storefront-sort-attribute-options",
    "navigate_to_catalog_search",
    '("attribute_code", attribute_code)',
    '("sort_attribute_code", sort_attribute_code)',
  ]) {
    assertContains(storefrontUi, marker, `${storefrontUiPath}: storefront Leptos catalog route/control marker missing ${marker}`);
  }
  assertNotContains(storefrontUi, "rustok_product", `${storefrontUiPath}: search storefront UI must consume host-provided catalog metadata, not product internals`);

  for (const marker of [
    "value label count",
    "channel_id: filters.channel_id",
    "category_ids: (!filters.category_ids.is_empty()).then_some(filters.category_ids)",
    "attribute_filters: (!filters.attribute_filters.is_empty())",
    "sort_attribute_code: filters.sort_attribute_code",
    "sort_desc: filters.sort_desc.then_some(true)",
    "SearchAttributeFilterInput",
  ]) {
    assertContains(storefrontGraphql, marker, `${storefrontGraphqlPath}: storefront GraphQL catalog transport marker missing ${marker}`);
  }

  for (const marker of [
    "value label count",
    "channel_id: filters.channel_id",
    "parse_optional_uuid(input.channel_id.as_deref())",
    "normalize_uuid_values(\"category_ids\", input.category_ids)",
    "normalize_attribute_filters(input.attribute_filters)",
    "normalize_attribute_code(input.sort_attribute_code)",
    "sort_desc: input.sort_desc.unwrap_or(false)",
    "label: bucket.label",
    "SearchAttributeFilterInput",
  ]) {
    assertContains(adminNative, marker, `${adminNativePath}: admin native catalog transport marker missing ${marker}`);
  }

  for (const marker of [
    "export type SearchCatalogFilterOption",
    "categoryOptions?: SearchCatalogFilterOption[]",
    "attributeOptions?: SearchCatalogFilterOption[]",
    "type SearchPreviewFiltersInput =",
    "channelId?: string",
    "categoryIds: string[]",
    "attributeFilters: Array<",
    "sortAttributeCode?: string",
    "facets { name buckets { value label count } }",
    "channelId: optionalText(channelId)",
    "categoryIds: parseCsv(categoryIds)",
    "sortAttributeCode: optionalText(sortAttributeCode)",
    "CatalogOptionDatalist",
    "search-admin-category-options",
    "search-admin-attribute-options",
    "bucket.label || bucket.value",
  ]) {
    assertContains(nextAdmin, marker, `${nextAdminPath}: Next admin catalog UI/GraphQL marker missing ${marker}`);
  }
  assertNotContains(nextAdmin, "rustok-product", `${nextAdminPath}: Next admin search package must consume host-provided catalog metadata, not product internals`);

  for (const marker of [
    "export type SearchCatalogFilterOption",
    "categoryOptions?: SearchCatalogFilterOption[]",
    "attributeOptions?: SearchCatalogFilterOption[]",
    "export type SearchCatalogFilters =",
    "initialFilters?: Partial<SearchCatalogFilters>",
    "facets {",
    "label",
    "channelId: filters.channelId.trim() || undefined",
    "categoryIds: categoryIds.length ? categoryIds : undefined",
    "attributeFilters: filters.attributeCode.trim()",
    "sortAttributeCode: filters.sortAttributeCode.trim() || undefined",
    "options?: SearchCatalogFilterOption[]",
    "React.useId()",
    "<datalist id={listId}>",
    "bucket.label || bucket.value",
    "CatalogField",
  ]) {
    assertContains(nextStorefront, marker, `${nextStorefrontPath}: Next storefront catalog UI/GraphQL marker missing ${marker}`);
  }
  assertNotContains(nextStorefront, "rustok-product", `${nextStorefrontPath}: Next storefront search package must consume host-provided catalog metadata, not product internals`);

  for (const marker of [
    "export type ProductCatalogSearchOption",
    "listCatalogCategorySearchOptions",
    "listCatalogAttributeSearchOptions",
    "productAttributes(tenantId: $tenantId, locale: $locale)",
    "catalogCategories(tenantId: $tenantId, locale: $locale)",
    "attribute.isFilterable || attribute.isSortable",
  ]) {
    assertContains(nextProduct, marker, `${nextProductPath}: product-owned search metadata marker missing ${marker}`);
  }

  for (const marker of [
    "export type ProductCatalogSearchOption",
    "export type ProductCatalogSearchOptions",
    "fetchCatalogSearchOptions",
    "StorefrontCatalogSearchOptions($locale: String!)",
    "storefrontCatalogSearchOptions(locale: $locale)",
    "categoryOptions { value label }",
    "attributeOptions { value label }",
    "request.locale.trim()",
    "tenant: request.tenantSlug",
  ]) {
    assertContains(nextStorefrontProduct, marker, `${nextStorefrontProductPath}: Next storefront product-owned metadata marker missing ${marker}`);
  }

  for (const marker of [
    "getLocale",
    "listCatalogCategorySearchOptions",
    "listCatalogAttributeSearchOptions",
    "loadCatalogSearchOptions",
    "locale",
    "categoryOptions={categoryOptions}",
    "attributeOptions={attributeOptions}",
  ]) {
    assertContains(nextSearchPage, marker, `${nextSearchPagePath}: Next host search composition marker missing ${marker}`);
  }

  for (const marker of [
    "SearchStorefrontPage",
    "SearchCatalogFilterOption",
    "fetchCatalogSearchOptions",
    "enabledModules.includes(\"product\")",
    "locale.trim()",
    "tenantSlug",
    "categoryOptions={categoryOptions}",
    "attributeOptions={attributeOptions}",
  ]) {
    assertContains(nextStorefrontSearchFeature, marker, `${nextStorefrontSearchFeaturePath}: Next storefront host catalog composition marker missing ${marker}`);
  }
  assertNotContains(nextStorefrontSearchFeature, '|| "en"', `${nextStorefrontSearchFeaturePath}: Next storefront host composition must not invent a locale fallback`);
  assertNotContains(nextStorefrontSearchFeature, "catalogCategories(", `${nextStorefrontSearchFeaturePath}: Next storefront host composition must use product-owned metadata helper`);
  assertContains(nextStorefrontModules, "@/features/search", `${nextStorefrontModulesPath}: Next storefront host must register search feature composition`);
  for (const marker of [
    "getStorefrontTenantSlug",
    "fetchEnabledModules(tenantSlug)",
    "module.render({ locale, enabledModules, tenantSlug })",
  ]) {
    assertContains(nextStorefrontHome, marker, `${nextStorefrontHomePath}: Next storefront home render context marker missing ${marker}`);
  }

  for (const marker of [
    "pub struct ProductCatalogSearchOption",
    "pub struct ProductCatalogSearchOptions",
    "pub category_options: Vec<ProductCatalogSearchOption>",
    "pub attribute_options: Vec<ProductCatalogSearchOption>",
  ]) {
    assertContains(productAdminModel, marker, `${productAdminModelPath}: product Leptos metadata model marker missing ${marker}`);
  }

  for (const marker of [
    "pub async fn fetch_catalog_search_options",
    "native_server_adapter::fetch_catalog_search_options(locale.clone())",
    "graphql_adapter::fetch_bootstrap",
    "bootstrap.current_tenant.id",
    "graphql_adapter::fetch_catalog_categories",
    "locale.clone()",
    "graphql_adapter::fetch_product_attributes",
    "attribute.is_filterable || attribute.is_sortable",
    "ProductCatalogSearchOption",
    "ProductCatalogSearchOptions",
  ]) {
    assertContains(productAdminTransport, marker, `${productAdminTransportPath}: product Leptos metadata helper marker missing ${marker}`);
  }

  for (const marker of [
    "pub use model::{ProductCatalogSearchOption, ProductCatalogSearchOptions}",
    "pub use transport::fetch_catalog_search_options",
  ]) {
    assertContains(productAdminLib, marker, `${productAdminLibPath}: product Leptos metadata export marker missing ${marker}`);
  }

  for (const marker of [
    "fetch_catalog_search_options(",
    "product_admin_catalog_search_options_native(locale)",
    'endpoint = "product/admin/catalog-search-options"',
    "native_context().await?",
    "Permission::PRODUCTS_READ",
    "list_categories(tenant.id, locale.trim())",
    "list_attributes(tenant.id, locale.trim())",
    "attribute.is_filterable || attribute.is_sortable",
  ]) {
    assertContains(productAdminNative, marker, `${productAdminNativePath}: product native catalog metadata marker missing ${marker}`);
  }

  for (const marker of [
    "fetch_catalog_search_options",
    "SearchAdmin",
    "SearchCatalogFilterOption",
    'use_is_module_enabled("product")',
    "UiRouteContext",
    "use_token()",
    "use_tenant()",
    "locale.trim().is_empty()",
    "category_options=category_options",
    "attribute_options=attribute_options",
  ]) {
    assertContains(adminComposition, marker, `${adminCompositionPath}: Leptos host catalog composition marker missing ${marker}`);
  }
  assertNotContains(adminComposition, 'unwrap_or_else(|| "en"', `${adminCompositionPath}: host composition must not invent a locale fallback`);

  for (const marker of ['entry.slug == "search"', "crate::app::modules::SearchAdminComposition"]) {
    assertContains(adminBuild, marker, `${adminBuildPath}: generated search host composition marker missing ${marker}`);
  }

  assertContains(
    readRepo("crates/rustok-search/admin/src/lib.rs"),
    "SearchCatalogFilterOption",
    "crates/rustok-search/admin/src/lib.rs: host-facing search option DTO must be public",
  );

  for (const marker of [
    "pub struct ProductCatalogSearchOption",
    "pub struct ProductCatalogSearchOptions",
    "pub category_options: Vec<ProductCatalogSearchOption>",
    "pub attribute_options: Vec<ProductCatalogSearchOption>",
  ]) {
    assertContains(productStorefrontModel, marker, `${productStorefrontModelPath}: product storefront metadata model marker missing ${marker}`);
  }

  for (const marker of [
    "pub async fn fetch_catalog_search_options",
    "native_server_adapter::fetch_catalog_search_options(locale.clone())",
    "graphql_adapter::fetch_catalog_search_options(locale)",
    "ProductTransportError::fallback_failed",
  ]) {
    assertContains(productStorefrontTransport, marker, `${productStorefrontTransportPath}: product storefront metadata transport marker missing ${marker}`);
  }

  for (const marker of [
    "pub async fn fetch_catalog_search_options",
    "storefront_catalog_search_options_native(locale)",
    'endpoint = "product/storefront/catalog-search-options"',
    "TenantContext",
    "ProductCatalogSchemaService",
    'locale.trim().is_empty()',
    '"locale is required"',
    "list_categories(tenant.id, locale.trim())",
    "list_attributes(tenant.id, locale.trim())",
    "attribute.is_filterable || attribute.is_sortable",
  ]) {
    assertContains(productStorefrontNative, marker, `${productStorefrontNativePath}: product storefront native metadata marker missing ${marker}`);
  }

  for (const marker of [
    "StorefrontCatalogSearchOptions($locale: String!)",
    "storefrontCatalogSearchOptions(locale: $locale)",
    "categoryOptions { value label }",
    "attributeOptions { value label }",
    "pub async fn fetch_catalog_search_options",
  ]) {
    assertContains(productStorefrontGraphql, marker, `${productStorefrontGraphqlPath}: product storefront GraphQL metadata marker missing ${marker}`);
  }

  for (const marker of [
    "pub use model::{ProductCatalogSearchOption, ProductCatalogSearchOptions}",
    "pub use transport::fetch_catalog_search_options",
  ]) {
    assertContains(productStorefrontLib, marker, `${productStorefrontLibPath}: product storefront metadata export marker missing ${marker}`);
  }

  for (const marker of [
    "async fn storefront_catalog_search_options",
    'require_module_enabled(ctx, "product")',
    "require_storefront_channel_enabled",
    "locale.trim().is_empty()",
    '"locale is required"',
    "ctx.data::<TenantContext>()",
    "list_categories(tenant.id, locale.trim())",
    "list_attributes(tenant.id, locale.trim())",
    "attribute.is_filterable || attribute.is_sortable",
  ]) {
    assertContains(commerceQuery, marker, `${commerceQueryPath}: public storefront catalog metadata GraphQL marker missing ${marker}`);
  }
  for (const marker of ["GqlProductCatalogSearchOption", "GqlProductCatalogSearchOptions"]) {
    assertContains(commerceTypes, marker, `${commerceTypesPath}: public storefront catalog metadata payload marker missing ${marker}`);
  }

  for (const marker of [
    "fetch_catalog_search_options",
    "SearchView",
    "SearchCatalogFilterOption",
    'use_is_module_enabled("product")',
    "UiRouteContext",
    "locale.trim().is_empty()",
    "category_options=category_options",
    "attribute_options=attribute_options",
  ]) {
    assertContains(storefrontComposition, marker, `${storefrontCompositionPath}: storefront host catalog composition marker missing ${marker}`);
  }
  assertNotContains(storefrontComposition, 'unwrap_or_else(|| "en"', `${storefrontCompositionPath}: storefront host composition must not invent a locale fallback`);
  for (const marker of ['entry.slug == "search"', "crate::modules::SearchStorefrontComposition"]) {
    assertContains(storefrontBuild, marker, `${storefrontBuildPath}: generated storefront search composition marker missing ${marker}`);
  }
  assertContains(searchStorefrontLib, "SearchCatalogFilterOption", `${searchStorefrontLibPath}: host-facing storefront search option DTO must be public`);
}

assertSearchAdminBoundary();
assertSearchStorefrontBoundary();
assertSearchCatalogProjectionContract();
assertSearchUiCatalogTransportContract();

if (failures.length > 0) {
  console.error("search UI boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("search UI boundary verification passed");
