#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-product-admin-boundary.mjs");

function writeFixtureFile(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function libSource() {
  return `
mod core;
mod i18n;
mod model;
mod transport;
mod ui;

pub use ui::leptos::ProductAdmin;
`;
}

function coreSource({ includeLeptos = false, omitOpenProduct = false } = {}) {
  return `
${includeLeptos ? "use leptos::prelude::*;" : ""}
pub(crate) struct ProductAdminSaveCommand;
pub(crate) struct ProductAdminEditorFormState;
pub(crate) struct ProductAdminStatusMutationResultViewModel;
pub(crate) struct ProductAdminDeleteResultViewModel;
pub(crate) struct ProductAdminSeoPanelCopy;
pub(crate) struct ProductAdminSummaryPanelCopy;
pub(crate) struct ProductAdminRouteQueryIntent;
pub(crate) enum ProductAdminSelectedProductQueryState { Open, Clear }
pub(crate) enum ProductAdminProductsLoadViewModel { State, Ready }
pub(crate) struct ProductAdminShippingProfilesLoadViewModel;
pub(crate) struct ProductAttributeEditorState;
pub(crate) struct ProductAdminListItemViewModel { pub show_shipping_profile: bool }
pub(crate) fn parse_product_admin_inventory_quantity_input(value: &str) -> i32 { 0 }
${omitOpenProduct ? "" : "pub(crate) enum ProductAdminOpenProductViewModel { Ready, Empty }"}
pub(crate) fn product_admin_pricing_preview_state_from_result() {}
pub(crate) fn product_admin_selected_product_query_state() -> ProductAdminSelectedProductQueryState { ProductAdminSelectedProductQueryState::Clear }
pub(crate) fn product_admin_products_load_view_from_result() -> ProductAdminProductsLoadViewModel { ProductAdminProductsLoadViewModel::State }
pub(crate) fn product_admin_shipping_profiles_load_view_from_result() -> ProductAdminShippingProfilesLoadViewModel { ProductAdminShippingProfilesLoadViewModel }
pub(crate) fn build_product_admin_summary_panel_copy() -> ProductAdminSummaryPanelCopy { ProductAdminSummaryPanelCopy }
`;
}

function uiSource({
  rawApiCall = false,
  rawServiceCall = false,
  directSummaryCopy = false,
  uiShippingProfilePolicy = false,
  uiSelectedQueryPolicy = false,
  uiProductsLoadPolicy = false,
  uiShippingProfilesLoadPolicy = false,
} = {}) {
  return `
use crate::core::{build_product_admin_save_command, build_product_admin_summary_panel_copy, ProductAdminOpenProductViewModel, product_admin_pricing_preview_state_from_result, product_admin_products_load_view_from_result, product_admin_selected_product_query_state, product_admin_shipping_profiles_load_view_from_result};
use crate::transport;

pub fn ProductAdmin() {
    let _transport = transport::fetch_products;
    let _save = build_product_admin_save_command;
    let _open = ProductAdminOpenProductViewModel::Empty;
    let _pricing = product_admin_pricing_preview_state_from_result;
    let _summary = build_product_admin_summary_panel_copy;
    let _query_state = product_admin_selected_product_query_state;
    let _products_load = product_admin_products_load_view_from_result;
    let _shipping_profiles_load = product_admin_shipping_profiles_load_view_from_result;
    let _typed_editor = TypedProductAttributeField;
    let _save_attribute_values = transport::save_product_attribute_values;
    let _clear_detached_values = transport::clear_detached_product_attribute_values;
    ${rawApiCall ? "let _raw = api::fetch_products;" : ""}
    ${rawServiceCall ? "let _service = ProductService::new;" : ""}
    ${directSummaryCopy ? 'let _copy = "Selected product";' : ""}
    ${uiShippingProfilePolicy ? "let item_shipping_profile_label = Some(String::new()); let _show = item_shipping_profile_label.is_some();" : ""}
    ${uiSelectedQueryPolicy ? "let product_id = String::new(); let _open = !product_id.trim().is_empty();" : ""}
    ${uiProductsLoadPolicy ? "let list = ProductList { items: Vec::new() }; if list.items.is_empty() {}" : ""}
    ${uiShippingProfilesLoadPolicy ? "let shipping_profiles = Resource; match shipping_profiles.get() { _ => {} }" : ""}
}
`;
}

function transportSource({ includeServerEndpoint = false } = {}) {
  return `
mod graphql_adapter;
mod native_server_adapter;

pub async fn fetch_bootstrap() { graphql_adapter::fetch_bootstrap().await; }
pub async fn fetch_products() { graphql_adapter::fetch_products().await; }
pub async fn fetch_product() { graphql_adapter::fetch_product().await; }
pub async fn fetch_product_pricing() { graphql_adapter::fetch_product_pricing().await; }
pub async fn fetch_shipping_profiles() { graphql_adapter::fetch_shipping_profiles().await; }
pub async fn fetch_product_attributes(locale: String) { graphql_adapter::fetch_product_attributes(locale).await; }
pub async fn fetch_catalog_categories(locale: String) { graphql_adapter::fetch_catalog_categories(locale).await; }
pub async fn fetch_attribute_schemas(locale: String) { graphql_adapter::fetch_attribute_schemas(locale).await; }
pub async fn fetch_effective_product_form(locale: String) { native_server_adapter::fetch_effective_product_form(locale).await; graphql_adapter::fetch_effective_product_form(locale).await; }
pub async fn fetch_product_attribute_values(locale: String) { native_server_adapter::fetch_product_attribute_values(locale).await; graphql_adapter::fetch_product_attribute_values(locale).await; }
pub async fn create_product() { graphql_adapter::create_product().await; }
pub async fn create_product_attribute(locale: String) { graphql_adapter::create_product_attribute(locale).await; }
pub async fn create_product_attribute_option(locale: String) { graphql_adapter::create_product_attribute_option(locale).await; }
pub async fn create_catalog_category(locale: String) { graphql_adapter::create_catalog_category(locale).await; }
pub async fn create_attribute_schema(locale: String) { graphql_adapter::create_attribute_schema(locale).await; }
pub async fn create_product_attribute_schema_group(locale: String) { graphql_adapter::create_product_attribute_schema_group(locale).await; }
pub async fn create_category_attribute_group(locale: String) { graphql_adapter::create_category_attribute_group(locale).await; }
pub async fn set_category_schema_mode() { graphql_adapter::set_category_schema_mode().await; }
pub async fn bind_schema_attribute() { graphql_adapter::bind_schema_attribute().await; }
pub async fn bind_category_attribute() { graphql_adapter::bind_category_attribute().await; }
pub async fn save_product_attribute_values(locale: String) { native_server_adapter::save_product_attribute_values(locale).await; graphql_adapter::save_product_attribute_values(locale).await; }
pub async fn clear_detached_product_attribute_values(locale: String) { native_server_adapter::clear_detached_product_attribute_values(locale).await; graphql_adapter::clear_detached_product_attribute_values(locale).await; }
pub async fn update_product() { graphql_adapter::update_product().await; }
pub async fn change_product_status() { graphql_adapter::change_product_status().await; }
pub async fn delete_product() { graphql_adapter::delete_product().await; }
${includeServerEndpoint ? '#[server(prefix = "/api/fn", endpoint = "bad")] async fn bad() {}' : ""}
`;
}

function nativeAdapterSource() {
  return `
use leptos::prelude::*;
use rustok_product::ProductCatalogSchemaService;

pub async fn fetch_effective_product_form(locale: String) {}
pub async fn fetch_product_attribute_values(locale: String) {}
pub async fn save_product_attribute_values(locale: String) {}
pub async fn clear_detached_product_attribute_values(locale: String) {}

#[server(prefix = "/api/fn", endpoint = "product/admin/attributes")]
async fn product_admin_attributes_native(tenant_id: String, locale: String) { let _service = ProductCatalogSchemaService::new; let _auth = leptos_axum::extract::<rustok_api::AuthContext>; let _tenant = leptos_axum::extract::<rustok_api::TenantContext>; }
#[server(prefix = "/api/fn", endpoint = "product/admin/categories")]
async fn product_admin_categories_native(tenant_id: String, locale: String) { let _service = ProductCatalogSchemaService::new; }
#[server(prefix = "/api/fn", endpoint = "product/admin/attribute-schemas")]
async fn product_admin_attribute_schemas_native(tenant_id: String, locale: String) { let _service = ProductCatalogSchemaService::new; }
#[server(prefix = "/api/fn", endpoint = "product/admin/effective-form")]
async fn product_admin_effective_form_native(tenant_id: String, locale: String) { let _service = ProductCatalogSchemaService::new; }
#[server(prefix = "/api/fn", endpoint = "product/admin/attribute-values")]
async fn product_admin_attribute_values_native(tenant_id: String, locale: String) { let _service = ProductCatalogSchemaService::new; }
#[server(prefix = "/api/fn", endpoint = "product/admin/save-attribute-values")]
async fn product_admin_save_attribute_values_native(tenant_id: String, locale: String) { let _service = ProductCatalogSchemaService::new; }
#[server(prefix = "/api/fn", endpoint = "product/admin/clear-detached-attribute-values")]
async fn product_admin_clear_detached_attribute_values_native(tenant_id: String, locale: String) { let _service = ProductCatalogSchemaService::new; }
#[server(prefix = "/api/fn", endpoint = "product/admin/create-attribute")]
async fn product_admin_create_attribute_native(tenant_id: String, locale: String) { let _service = ProductCatalogSchemaService::new; }
#[server(prefix = "/api/fn", endpoint = "product/admin/create-attribute-option")]
async fn product_admin_create_attribute_option_native(tenant_id: String, locale: String) { let _service = ProductCatalogSchemaService::new; }
#[server(prefix = "/api/fn", endpoint = "product/admin/create-category")]
async fn product_admin_create_category_native(tenant_id: String, locale: String) { let _service = ProductCatalogSchemaService::new; }
#[server(prefix = "/api/fn", endpoint = "product/admin/create-attribute-schema")]
async fn product_admin_create_schema_native(tenant_id: String, locale: String) { let _service = ProductCatalogSchemaService::new; }
#[server(prefix = "/api/fn", endpoint = "product/admin/create-schema-group")]
async fn product_admin_create_schema_group_native(tenant_id: String, locale: String) { let _service = ProductCatalogSchemaService::new; }
#[server(prefix = "/api/fn", endpoint = "product/admin/create-category-group")]
async fn product_admin_create_category_group_native(tenant_id: String, locale: String) { let _service = ProductCatalogSchemaService::new; }
#[server(prefix = "/api/fn", endpoint = "product/admin/set-category-schema-mode")]
async fn product_admin_set_category_schema_mode_native(tenant_id: String) { let _service = ProductCatalogSchemaService::new; }
#[server(prefix = "/api/fn", endpoint = "product/admin/bind-schema-attribute")]
async fn product_admin_bind_schema_attribute_native(tenant_id: String) { let _service = ProductCatalogSchemaService::new; }
#[server(prefix = "/api/fn", endpoint = "product/admin/bind-category-attribute")]
async fn product_admin_bind_category_attribute_native(tenant_id: String) { let _service = ProductCatalogSchemaService::new; }
`;
}

function apiSource({ optionalCatalogLocale = false, fallbackCatalogLocale = false } = {}) {
  return `
use rustok_graphql::GraphqlRequest;
const PRODUCT_ATTRIBUTES_QUERY: &str = "query ProductAdminAttributes($tenantId: UUID!, $locale: String!) { productAttributes(tenantId: $tenantId, locale: $locale) { total items { id } } }";
const CATALOG_CATEGORIES_QUERY: &str = "query ProductAdminCatalogCategories($tenantId: UUID!, $locale: String!) { catalogCategories(tenantId: $tenantId, locale: $locale) { total items { id } } }";
const ATTRIBUTE_SCHEMAS_QUERY: &str = "query ProductAdminAttributeSchemas($tenantId: UUID!, $locale: String!) { productAttributeSchemas(tenantId: $tenantId, locale: $locale) { total items { id } } }";
const EFFECTIVE_FORM_QUERY: &str = "query ProductAdminEffectiveForm($tenantId: UUID!, $productId: UUID, $categoryId: UUID, $locale: String!) { productEffectiveForm(tenantId: $tenantId, productId: $productId, categoryId: $categoryId, locale: $locale) { categoryId attributes { options { id code label position } groupCode groupLabel } } }";
const ATTRIBUTE_VALUES_QUERY: &str = "query ProductAdminAttributeValues($tenantId: UUID!, $productId: UUID!, $locale: String!) { productAttributeValues(tenantId: $tenantId, productId: $productId, locale: $locale) { attributeId } }";
const SAVE_ATTRIBUTE_VALUES_MUTATION: &str = "mutation ProductAdminSaveAttributeValues($tenantId: UUID!, $userId: UUID!, $productId: UUID!, $locale: String!, $patches: [ProductAttributeValuePatchInput!]!) { saveProductAttributeValues(tenantId: $tenantId, userId: $userId, productId: $productId, locale: $locale, patches: $patches) { attributeId } }";
const CLEAR_DETACHED_ATTRIBUTE_VALUES_MUTATION: &str = "mutation ProductAdminClearDetachedAttributeValues($tenantId: UUID!, $userId: UUID!, $productId: UUID!, $locale: String!, $attributeIds: [UUID!]!) { clearDetachedProductAttributeValues(tenantId: $tenantId, userId: $userId, productId: $productId, locale: $locale, attributeIds: $attributeIds) { attributeId } }";
const CREATE_PRODUCT_ATTRIBUTE_MUTATION: &str = "mutation ProductAdminCreateAttribute($tenantId: UUID!, $userId: UUID!, $locale: String!, $input: CreateProductAttributeInput!) { createProductAttribute(tenantId: $tenantId, userId: $userId, locale: $locale, input: $input) }";
const CREATE_PRODUCT_ATTRIBUTE_OPTION_MUTATION: &str = "mutation ProductAdminCreateAttributeOption($tenantId: UUID!, $userId: UUID!, $locale: String!, $input: CreateProductAttributeOptionInput!) { createProductAttributeOption(tenantId: $tenantId, userId: $userId, locale: $locale, input: $input) }";
const CREATE_CATALOG_CATEGORY_MUTATION: &str = "mutation ProductAdminCreateCatalogCategory($tenantId: UUID!, $userId: UUID!, $locale: String!, $input: CreateCatalogCategoryInput!) { createCatalogCategory(tenantId: $tenantId, userId: $userId, locale: $locale, input: $input) }";
const CREATE_ATTRIBUTE_SCHEMA_MUTATION: &str = "mutation ProductAdminCreateAttributeSchema($tenantId: UUID!, $userId: UUID!, $locale: String!, $input: CreateProductAttributeSchemaInput!) { createProductAttributeSchema(tenantId: $tenantId, userId: $userId, locale: $locale, input: $input) }";
const CREATE_SCHEMA_GROUP_MUTATION: &str = "mutation ProductAdminCreateSchemaGroup($tenantId: UUID!, $userId: UUID!, $locale: String!, $input: CreateProductAttributeSchemaGroupInput!) { createProductAttributeSchemaGroup(tenantId: $tenantId, userId: $userId, locale: $locale, input: $input) }";
const CREATE_CATEGORY_GROUP_MUTATION: &str = "mutation ProductAdminCreateCategoryGroup($tenantId: UUID!, $userId: UUID!, $locale: String!, $input: CreateCategoryAttributeGroupInput!) { createCatalogCategoryAttributeGroup(tenantId: $tenantId, userId: $userId, locale: $locale, input: $input) }";
struct LocaleVariables { locale: String }
struct LocaleMutationVariables<T> { locale: String, input: T }
pub async fn fetch_bootstrap() {}
pub async fn fetch_products() {}
pub async fn fetch_product() {}
pub async fn fetch_product_pricing() {}
pub async fn fetch_shipping_profiles() {}
pub async fn fetch_product_attributes(${optionalCatalogLocale ? "locale: Option<String>" : "locale: String"}) { ${fallbackCatalogLocale ? 'let _locale = locale.unwrap_or_else(|| "en".to_string());' : ""} }
pub async fn fetch_catalog_categories(locale: String) {}
pub async fn fetch_attribute_schemas(locale: String) {}
pub async fn fetch_effective_product_form(locale: String) {}
pub async fn fetch_product_attribute_values(locale: String) {}
pub async fn create_product() {}
pub async fn create_product_attribute(locale: String) {}
pub async fn create_product_attribute_option(locale: String) {}
pub async fn create_catalog_category(locale: String) {}
pub async fn create_attribute_schema(locale: String) {}
pub async fn create_product_attribute_schema_group(locale: String) {}
pub async fn create_category_attribute_group(locale: String) {}
pub async fn set_category_schema_mode() {}
pub async fn bind_schema_attribute() {}
pub async fn bind_category_attribute() {}
pub async fn save_product_attribute_values(locale: String) {}
pub async fn clear_detached_product_attribute_values(locale: String) {}
pub async fn update_product() {}
pub async fn change_product_status() {}
pub async fn delete_product() {}
`;
}

function commerceQuerySource() {
  return `
use rustok_product::ProductCatalogSchemaService;

pub struct CommerceQuery;

impl CommerceQuery {
    async fn product_attributes(&self, locale: String) { let _service = ProductCatalogSchemaService::new; }
    async fn catalog_categories(&self, locale: String) { let _service = ProductCatalogSchemaService::new; }
    async fn product_attribute_schemas(&self, locale: String) { let _service = ProductCatalogSchemaService::new; }
    async fn product_effective_form(&self, locale: String) { let _service = ProductCatalogSchemaService::new; }
    async fn product_attribute_values(&self, locale: String) { let _service = ProductCatalogSchemaService::new; }
}
`;
}

function commerceCatalogMutationSource() {
  return `
use rustok_product::ProductCatalogSchemaService;

pub struct CommerceCatalogMutation;

impl CommerceCatalogMutation {
    async fn create_product_attribute(&self, locale: String) { let _service = ProductCatalogSchemaService::new; }
    async fn create_product_attribute_option(&self, locale: String) { let _service = ProductCatalogSchemaService::new; }
    async fn create_catalog_category(&self, locale: String) { let _service = ProductCatalogSchemaService::new; }
    async fn create_product_attribute_schema(&self, locale: String) { let _service = ProductCatalogSchemaService::new; }
    async fn create_product_attribute_schema_group(&self, locale: String) { let _service = ProductCatalogSchemaService::new; }
    async fn create_catalog_category_attribute_group(&self, locale: String) { let _service = ProductCatalogSchemaService::new; }
    async fn set_catalog_category_schema_mode(&self) { let _service = ProductCatalogSchemaService::new; }
    async fn bind_product_attribute_schema_attribute(&self) { let _service = ProductCatalogSchemaService::new; }
    async fn bind_catalog_category_attribute(&self) { let _service = ProductCatalogSchemaService::new; }
    async fn save_product_attribute_values(&self, locale: String) { let _service = ProductCatalogSchemaService::new; }
    async fn clear_detached_product_attribute_values(&self, locale: String) { let _service = ProductCatalogSchemaService::new; }
}
`;
}

function commerceTypesSource() {
  return `
pub struct GqlProductAttributeList;
pub struct GqlCatalogCategoryList;
pub struct GqlProductAttributeSchemaList;
pub struct GqlProductEffectiveForm;
pub struct GqlProductAttributeOption;
pub struct group_label;
pub struct GqlProductAttributeValue;
pub struct ProductAttributeValuePatchInput;
pub struct CreateProductAttributeInput;
pub struct CreateProductAttributeOptionInput;
pub struct CreateCatalogCategoryInput;
pub struct CreateProductAttributeSchemaInput;
pub struct CreateProductAttributeSchemaGroupInput;
pub struct CreateCategoryAttributeGroupInput;
pub struct SetCategorySchemaModeInput;
pub struct BindSchemaAttributeInput;
pub struct BindCategoryAttributeInput;
`;
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-boundary-"));
  writeFixtureFile(root, "crates/rustok-product/admin/src/lib.rs", libSource());
  writeFixtureFile(root, "crates/rustok-product/admin/src/core.rs", coreSource(options));
  writeFixtureFile(root, "crates/rustok-product/admin/src/ui/leptos.rs", uiSource(options));
  writeFixtureFile(root, "crates/rustok-product/admin/src/transport.rs", transportSource(options));
  writeFixtureFile(root, "crates/rustok-product/admin/src/transport/graphql_adapter.rs", apiSource(options));
  writeFixtureFile(root, "crates/rustok-product/admin/src/transport/native_server_adapter.rs", nativeAdapterSource());
  if (options.legacyApi) writeFixtureFile(root, "crates/rustok-product/admin/src/api.rs", apiSource(options));
  writeFixtureFile(root, "crates/rustok-commerce/src/graphql/query.rs", commerceQuerySource());
  writeFixtureFile(root, "crates/rustok-commerce/src/graphql/mutations/catalog.rs", commerceCatalogMutationSource());
  writeFixtureFile(root, "crates/rustok-commerce/src/graphql/types.rs", commerceTypesSource());
  writeFixtureFile(root, "crates/rustok-product/docs/implementation-plan.md", "verify-product-admin-boundary.mjs category-bound admin transport");
  writeFixtureFile(root, "docs/modules/registry.md", "verify-product-admin-boundary.mjs category-bound admin transport");
  writeFixtureFile(root, "package.json", JSON.stringify({
    scripts: {
      "verify:product:admin-boundary": "node scripts/verify/verify-product-admin-boundary.mjs",
      "test:verify:product:admin-boundary": "node scripts/verify/verify-product-admin-boundary.test.mjs",
      "test:verify:ffa:ui:migration": "npm run test:verify:product:admin-boundary",
    },
  }));
  return root;
}

function runVerifier(root) {
  return spawnSync("node", [scriptPath], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

test("product admin boundary verifier passes canonical fixture", () => {
  const root = withFixture();
  try {
    const result = runVerifier(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /product admin boundary verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("product admin boundary verifier rejects Leptos-specific core", () => {
  const root = withFixture({ includeLeptos: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected Leptos core fixture to fail");
    assert.match(result.stderr, /core must stay Leptos\/server-function free/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("product admin boundary verifier rejects raw api calls from UI", () => {
  const root = withFixture({ rawApiCall: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected raw UI api fixture to fail");
    assert.match(result.stderr, /UI adapter must not call raw transport or services/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("product admin boundary verifier rejects legacy api module", () => {
  const root = withFixture({ legacyApi: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected legacy api fixture to fail");
    assert.match(result.stderr, /legacy api\.rs/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("product admin boundary verifier rejects missing core open-result policy", () => {
  const root = withFixture({ omitOpenProduct: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected missing open-result helper fixture to fail");
    assert.match(result.stderr, /ProductAdminOpenProductViewModel/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("product admin boundary verifier rejects selected-summary copy in UI", () => {
  const root = withFixture({ directSummaryCopy: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected direct summary copy fixture to fail");
    assert.match(result.stderr, /selected-summary panel copy must stay in core/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("product admin boundary verifier rejects shipping profile chip policy in UI", () => {
  const root = withFixture({ uiShippingProfilePolicy: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected UI shipping-profile policy fixture to fail");
    assert.match(result.stderr, /shipping-profile chip display policy must stay in core/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("product admin boundary verifier rejects selected product query policy in UI", () => {
  const root = withFixture({ uiSelectedQueryPolicy: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected selected query policy fixture to fail");
    assert.match(result.stderr, /selected product query normalization must stay in core/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("product admin boundary verifier rejects products load-result policy in UI", () => {
  const root = withFixture({ uiProductsLoadPolicy: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected products load-result policy fixture to fail");
    assert.match(result.stderr, /products load-result normalization must stay in core/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("product admin boundary verifier rejects duplicated shipping-profile load policy in UI", () => {
  const root = withFixture({ uiShippingProfilesLoadPolicy: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected shipping-profile load policy fixture to fail");
    assert.match(result.stderr, /shipping-profile consumers must share core-owned load-result normalization/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("product admin boundary verifier rejects server functions in transport facade", () => {
  const root = withFixture({ includeServerEndpoint: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected transport server-function fixture to fail");
    assert.match(result.stderr, /server\/native endpoints must not live in the product admin transport facade/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("product admin boundary verifier rejects optional catalog locale in GraphQL adapter", () => {
  const root = withFixture({ optionalCatalogLocale: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected optional catalog locale fixture to fail");
    assert.match(result.stderr, /must not invent optional\/fallback locale/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("product admin boundary verifier rejects catalog locale fallback in GraphQL adapter", () => {
  const root = withFixture({ optionalCatalogLocale: true, fallbackCatalogLocale: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected catalog locale fallback fixture to fail");
    assert.match(result.stderr, /must not invent optional\/fallback locale/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
