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
  for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "leptos_graphql::", "execute_graphql", "#[server"] ) {
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
  assertContains(native, "leptos_graphql", `${nativePath}: admin raw adapter must keep GraphQL fallback implementation`);
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
  for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "native_server_adapter::", "graphql_adapter::", "leptos_graphql::", "execute_graphql", "#[server"] ) {
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

assertSearchAdminBoundary();
assertSearchStorefrontBoundary();
assertSearchCatalogProjectionContract();

if (failures.length > 0) {
  console.error("search UI boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("search UI boundary verification passed");
