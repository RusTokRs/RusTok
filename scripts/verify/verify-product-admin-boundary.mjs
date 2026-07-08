#!/usr/bin/env node
// RusTok product admin FFA boundary guardrails.
// Fast source-level checks for the module-owned core/transport/ui split.

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

const libPath = "crates/rustok-product/admin/src/lib.rs";
const corePath = "crates/rustok-product/admin/src/core.rs";
const uiPath = "crates/rustok-product/admin/src/ui/leptos.rs";
const transportPath = "crates/rustok-product/admin/src/transport.rs";
const legacyApiPath = "crates/rustok-product/admin/src/api.rs";
const graphqlAdapterPath = "crates/rustok-product/admin/src/transport/graphql_adapter.rs";
const nativeAdapterPath = "crates/rustok-product/admin/src/transport/native_server_adapter.rs";
const cargoPath = "crates/rustok-product/admin/Cargo.toml";
const commerceQueryPath = "crates/rustok-commerce/src/graphql/query.rs";
const commerceCatalogMutationPath = "crates/rustok-commerce/src/graphql/mutations/catalog.rs";
const commerceTypesPath = "crates/rustok-commerce/src/graphql/types.rs";
const implementationPlanPath = "crates/rustok-product/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";
const packagePath = "package.json";

for (const filePath of [
  libPath,
  corePath,
  uiPath,
  transportPath,
  graphqlAdapterPath,
  nativeAdapterPath,
  cargoPath,
  commerceQueryPath,
  commerceCatalogMutationPath,
  commerceTypesPath,
  implementationPlanPath,
  registryPath,
  packagePath,
]) {
  assertExists(filePath, `${filePath}: expected product admin FFA boundary file`);
}
if (existsSync(repoPath(legacyApiPath))) {
  fail(`${legacyApiPath}: product admin legacy api.rs must stay removed; transport/graphql_adapter.rs owns GraphQL operations`);
}

const lib = readRepo(libPath);
const core = readRepo(corePath);
const ui = readRepo(uiPath);
const transport = readRepo(transportPath);
const graphqlAdapter = readRepo(graphqlAdapterPath);
const nativeAdapter = readRepo(nativeAdapterPath);
const cargo = readRepo(cargoPath);
const commerceQuery = readRepo(commerceQueryPath);
const commerceCatalogMutation = readRepo(commerceCatalogMutationPath);
const commerceTypes = readRepo(commerceTypesPath);
const implementationPlan = readRepo(implementationPlanPath);
const registry = readRepo(registryPath);
const packageJson = readRepo(packagePath);

assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
assertContains(lib, "pub use ui::leptos::ProductAdmin;", `${libPath}: crate root must re-export ProductAdmin`);
assertNotContains(lib, "mod api;", `${libPath}: crate root must not wire legacy api adapter`);

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server", "LocalResource", "WriteSignal", "web_sys::"]) {
  assertNotContains(core, marker, `${corePath}: core must stay Leptos/server-function free (${marker})`);
}
for (const marker of [
  "SaveCommand",
  "ProductAdminEditorFormState",
  "StatusResultViewModel",
  "DeleteResultViewModel",
  "ProductAdminSeoPanelCopy",
  "ProductAdminSummaryPanelCopy",
  "parse_product_admin_inventory_quantity_input",
  "ProductAdminOpenProductViewModel",
  "pricing_preview_state_from_result",
  "ProductAdminRouteQueryIntent",
  "ProductAdminSelectedProductQueryState",
  "product_admin_selected_product_query_state",
  "ProductAdminProductsLoadViewModel",
  "product_admin_products_load_view_from_result",
  "ShippingProfilesLoadViewModel",
  "ProductAttributeEditorState",
  "shipping_profiles_load_view_from_result",
  "show_shipping_profile",
]) {
  assertContains(core, marker, `${corePath}: expected core-owned FFA helper ${marker}`);
}

assertContains(ui, "use crate::core::{", `${uiPath}: Leptos adapter must import core-owned helpers`);
assertContains(ui, "use crate::transport;", `${uiPath}: Leptos adapter must call the module-owned transport facade`);
assertContains(ui, "build_save_command", `${uiPath}: UI must use core-owned save command preparation`);
assertContains(ui, "ProductAdminOpenProductViewModel", `${uiPath}: UI must consume core-owned open-product outcomes`);
assertContains(ui, "pricing_preview_state_from_result", `${uiPath}: UI must use core-owned pricing preview state mapping`);
assertContains(ui, "build_product_admin_summary_panel_copy", `${uiPath}: UI must consume core-owned selected-summary panel copy`);
assertContains(ui, "product_admin_selected_product_query_state", `${uiPath}: UI must use core-owned selected product query state`);
assertContains(ui, "product_admin_products_load_view_from_result", `${uiPath}: UI must use core-owned products load-result normalization`);
assertContains(ui, "shipping_profiles_load_view_from_result", `${uiPath}: UI must use core-owned shipping-profiles load-result normalization`);
for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "#[server", "ProductService", "PricingService"] ) {
  assertNotContains(ui, marker, `${uiPath}: UI adapter must not call raw transport or services (${marker})`);
}
for (const marker of ["product.summary.title", "Selected product"]) {
  assertNotContains(ui, marker, `${uiPath}: selected-summary panel copy must stay in core (${marker})`);
}
for (const marker of ["item_shipping_profile_label.is_some", "item_shipping_profile_label.clone().unwrap_or_default"]) {
  assertNotContains(ui, marker, `${uiPath}: shipping-profile chip display policy must stay in core (${marker})`);
}
for (const marker of ["product_id.trim().is_empty()", "selected_product_query.get() {"]) {
  assertNotContains(ui, marker, `${uiPath}: selected product query normalization must stay in core (${marker})`);
}
for (const marker of ["list.items.is_empty()", "list.items.into_iter().map"] ) {
  assertNotContains(ui, marker, `${uiPath}: products load-result normalization must stay in core (${marker})`);
}
assertNotContains(ui, "match shipping_profiles.get()", `${uiPath}: shipping-profile consumers must share core-owned load-result normalization`);
assertContains(ui, "TypedProductAttributeField", `${uiPath}: effective product form must render typed attribute editors`);
assertContains(ui, "save_product_attribute_values", `${uiPath}: product submit flow must persist typed attribute patches`);

for (const marker of [
  "fetch_bootstrap",
  "fetch_products",
  "fetch_product",
  "fetch_product_pricing",
  "fetch_shipping_profiles",
  "fetch_product_attributes",
  "fetch_catalog_categories",
  "fetch_attribute_schemas",
  "fetch_effective_product_form",
  "fetch_product_attribute_values",
  "create_product",
  "create_product_attribute",
  "create_product_attribute_option",
  "create_catalog_category",
  "create_attribute_schema",
  "create_product_attribute_schema_group",
  "create_category_attribute_group",
  "set_category_schema_mode",
  "bind_schema_attribute",
  "bind_category_attribute",
  "save_product_attribute_values",
  "clear_detached_product_attribute_values",
  "update_product",
  "change_product_status",
  "delete_product",
]) {
  assertContains(transport, marker, `${transportPath}: transport facade must expose ${marker}`);
}
assertContains(transport, "mod graphql_adapter;", `${transportPath}: transport facade must wire GraphQL adapter`);
assertContains(transport, "mod native_server_adapter;", `${transportPath}: transport facade must wire native server adapter`);
assertContains(transport, "graphql_adapter::fetch_products", `${transportPath}: transport facade must delegate through GraphQL adapter`);
assertContains(transport, "graphql_adapter::fetch_effective_product_form", `${transportPath}: catalog schema operations must delegate through GraphQL adapter`);
assertContains(transport, "native_server_adapter::fetch_effective_product_form", `${transportPath}: catalog schema operations must try native server adapter first`);
assertContains(transport, "native_server_adapter::save_product_attribute_values", `${transportPath}: typed attribute writes must try native server adapter first`);
assertNotContains(transport, "use crate::api", `${transportPath}: transport facade must not delegate to legacy api module`);
assertNotContains(transport, "#[server", `${transportPath}: server/native endpoints must not live in the product admin transport facade`);
assertContains(graphqlAdapter, "GraphqlRequest", `${graphqlAdapterPath}: product admin GraphQL adapter must keep the GraphQL transport contract`);
for (const marker of [
  "#[server",
  "ProductCatalogSchemaService",
  "product_admin_attributes_native",
  "product_admin_categories_native",
  "product_admin_attribute_schemas_native",
  "product_admin_effective_form_native",
  "product_admin_attribute_values_native",
  "product_admin_save_attribute_values_native",
  "product_admin_clear_detached_attribute_values_native",
  "product_admin_create_attribute_native",
  "product_admin_create_attribute_option_native",
  "product_admin_create_category_native",
  "product_admin_create_schema_native",
  "product_admin_create_schema_group_native",
  "product_admin_create_category_group_native",
  "product_admin_set_category_schema_mode_native",
  "product_admin_bind_schema_attribute_native",
  "product_admin_bind_category_attribute_native",
  "locale: String",
  "leptos_axum::extract::<rustok_api::AuthContext>",
  "leptos_axum::extract::<rustok_api::TenantContext>",
  "expect_context::<rustok_api::HostRuntimeContext>()",
  "shared_get::<rustok_outbox::TransactionalEventBus>()",
  "runtime_ctx.db_clone()",
]) {
  assertContains(nativeAdapter, marker, `${nativeAdapterPath}: native server adapter must expose category-bound server function contract (${marker})`);
}
for (const marker of ["loco_rs", "rustok_outbox::loco"]) {
  assertNotContains(nativeAdapter, marker, `${nativeAdapterPath}: native server adapter must not depend on Loco (${marker})`);
}
for (const marker of ["loco-rs", "loco-adapter"]) {
  assertNotContains(cargo, marker, `${cargoPath}: product admin package must not depend on Loco (${marker})`);
}
for (const marker of [/locale: Option<String>/, /unwrap_or_else\(\|\| "en"/, /PLATFORM_FALLBACK_LOCALE/]) {
  assertNotContains(nativeAdapter, marker, `${nativeAdapterPath}: native category-bound adapter must not invent optional/fallback locale`);
}
for (const marker of [
  "ProductAdminAttributes($tenantId: UUID!, $locale: String!)",
  "ProductAdminCatalogCategories($tenantId: UUID!, $locale: String!)",
  "ProductAdminAttributeSchemas($tenantId: UUID!, $locale: String!)",
  "ProductAdminEffectiveForm($tenantId: UUID!, $productId: UUID, $categoryId: UUID, $locale: String!)",
  "ProductAdminAttributeValues($tenantId: UUID!, $productId: UUID!, $locale: String!)",
  "ProductAdminSaveAttributeValues($tenantId: UUID!, $userId: UUID!, $productId: UUID!, $locale: String!",
  "ProductAdminClearDetachedAttributeValues($tenantId: UUID!, $userId: UUID!, $productId: UUID!, $locale: String!",
  "ProductAdminCreateAttribute($tenantId: UUID!, $userId: UUID!, $locale: String!",
  "ProductAdminCreateAttributeOption($tenantId: UUID!, $userId: UUID!, $locale: String!",
  "ProductAdminCreateCatalogCategory($tenantId: UUID!, $userId: UUID!, $locale: String!",
  "ProductAdminCreateAttributeSchema($tenantId: UUID!, $userId: UUID!, $locale: String!",
  "ProductAdminCreateSchemaGroup($tenantId: UUID!, $userId: UUID!, $locale: String!",
  "ProductAdminCreateCategoryGroup($tenantId: UUID!, $userId: UUID!, $locale: String!",
  "options { id code label position } groupCode groupLabel",
  "struct LocaleVariables",
  "struct LocaleMutationVariables",
]) {
  assertContains(graphqlAdapter, marker, `${graphqlAdapterPath}: new catalog attribute contract must use explicit host-provided locale (${marker})`);
}
for (const marker of [
  /fn fetch_product_attributes\([^)]*locale: Option<String>/,
  /fn fetch_catalog_categories\([^)]*locale: Option<String>/,
  /fn fetch_attribute_schemas\([^)]*locale: Option<String>/,
  /fn fetch_effective_product_form\([^)]*locale: Option<String>/,
  /fn fetch_product_attribute_values\([^)]*locale: Option<String>/,
  /fn save_product_attribute_values\([^)]*locale: Option<String>/,
  /fn create_product_attribute\([^)]*locale: Option<String>/,
  /fn create_catalog_category\([^)]*locale: Option<String>/,
  /fn create_attribute_schema\([^)]*locale: Option<String>/,
  /unwrap_or_else\(\|\| "en"/,
  /PLATFORM_FALLBACK_LOCALE/,
]) {
  assertNotContains(transport, marker, `${transportPath}: new catalog attribute facade must not invent optional/fallback locale`);
  assertNotContains(graphqlAdapter, marker, `${graphqlAdapterPath}: new catalog attribute GraphQL adapter must not invent optional/fallback locale`);
}
for (const marker of [
  "ProductCatalogSchemaService",
  "async fn product_attributes(",
  "async fn catalog_categories(",
  "async fn product_attribute_schemas(",
  "async fn product_effective_form(",
  "async fn product_attribute_values(",
  "locale: String",
]) {
  assertContains(commerceQuery, marker, `${commerceQueryPath}: server GraphQL query surface must expose category-bound catalog schema reads (${marker})`);
}
for (const marker of [
  "ProductCatalogSchemaService",
  "async fn create_product_attribute(",
  "async fn create_product_attribute_option(",
  "async fn create_catalog_category(",
  "async fn create_product_attribute_schema(",
  "async fn create_product_attribute_schema_group(",
  "async fn create_catalog_category_attribute_group(",
  "async fn set_catalog_category_schema_mode(",
  "async fn bind_product_attribute_schema_attribute(",
  "async fn bind_catalog_category_attribute(",
  "async fn save_product_attribute_values(",
  "async fn clear_detached_product_attribute_values(",
  "locale: String",
]) {
  assertContains(commerceCatalogMutation, marker, `${commerceCatalogMutationPath}: server GraphQL mutation surface must expose category-bound catalog schema writes (${marker})`);
}
for (const marker of [
  "GqlProductAttributeList",
  "GqlCatalogCategoryList",
  "GqlProductAttributeSchemaList",
  "GqlProductEffectiveForm",
  "GqlProductAttributeOption",
  "group_label",
  "GqlProductAttributeValue",
  "ProductAttributeValuePatchInput",
  "CreateProductAttributeInput",
  "CreateProductAttributeOptionInput",
  "CreateCatalogCategoryInput",
  "CreateProductAttributeSchemaInput",
  "CreateProductAttributeSchemaGroupInput",
  "CreateCategoryAttributeGroupInput",
  "SetCategorySchemaModeInput",
  "BindSchemaAttributeInput",
  "BindCategoryAttributeInput",
]) {
  assertContains(commerceTypes, marker, `${commerceTypesPath}: server GraphQL type surface must include category-bound catalog schema DTO/input ${marker}`);
}

assertContains(implementationPlan, "verify-product-admin-boundary.mjs", `${implementationPlanPath}: local plan must mention the product fast boundary guardrail`);
assertContains(implementationPlan, "category-bound admin transport", `${implementationPlanPath}: local plan must record category-bound admin transport evidence`);
assertContains(registry, "verify-product-admin-boundary.mjs", `${registryPath}: central readiness board must mention the product fast boundary guardrail`);
assertContains(registry, "category-bound admin transport", `${registryPath}: central readiness board must record category-bound admin transport evidence`);
assertContains(packageJson, "verify:product:admin-boundary", `${packagePath}: package scripts must expose product admin boundary verification`);
assertContains(packageJson, "test:verify:product:admin-boundary", `${packagePath}: package scripts must expose product admin boundary fixture tests`);
assertContains(packageJson, "npm run test:verify:product:admin-boundary", `${packagePath}: aggregate FFA fixture coverage must include product admin boundary tests`);

if (failures.length > 0) {
  console.error("product admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("product admin boundary verification passed");
