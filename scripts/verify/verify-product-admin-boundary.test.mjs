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

pub async fn fetch_bootstrap() { graphql_adapter::fetch_bootstrap().await; }
pub async fn fetch_products() { graphql_adapter::fetch_products().await; }
pub async fn fetch_product() { graphql_adapter::fetch_product().await; }
pub async fn fetch_product_pricing() { graphql_adapter::fetch_product_pricing().await; }
pub async fn fetch_shipping_profiles() { graphql_adapter::fetch_shipping_profiles().await; }
pub async fn create_product() { graphql_adapter::create_product().await; }
pub async fn update_product() { graphql_adapter::update_product().await; }
pub async fn change_product_status() { graphql_adapter::change_product_status().await; }
pub async fn delete_product() { graphql_adapter::delete_product().await; }
${includeServerEndpoint ? '#[server(prefix = "/api/fn", endpoint = "bad")] async fn bad() {}' : ""}
`;
}

function apiSource() {
  return `
use leptos_graphql::GraphqlRequest;
pub async fn fetch_bootstrap() {}
pub async fn fetch_products() {}
pub async fn fetch_product() {}
pub async fn fetch_product_pricing() {}
pub async fn fetch_shipping_profiles() {}
pub async fn create_product() {}
pub async fn update_product() {}
pub async fn change_product_status() {}
pub async fn delete_product() {}
`;
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-boundary-"));
  writeFixtureFile(root, "crates/rustok-product/admin/src/lib.rs", libSource());
  writeFixtureFile(root, "crates/rustok-product/admin/src/core.rs", coreSource(options));
  writeFixtureFile(root, "crates/rustok-product/admin/src/ui/leptos.rs", uiSource(options));
  writeFixtureFile(root, "crates/rustok-product/admin/src/transport.rs", transportSource(options));
  writeFixtureFile(root, "crates/rustok-product/admin/src/transport/graphql_adapter.rs", apiSource());
  if (options.legacyApi) writeFixtureFile(root, "crates/rustok-product/admin/src/api.rs", apiSource());
  writeFixtureFile(root, "crates/rustok-product/docs/implementation-plan.md", "verify-product-admin-boundary.mjs");
  writeFixtureFile(root, "docs/modules/registry.md", "verify-product-admin-boundary.mjs");
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
