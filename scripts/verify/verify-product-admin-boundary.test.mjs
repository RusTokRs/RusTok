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

function fixtureSources(options = {}) {
  return {
    "crates/rustok-product/admin/src/lib.rs": `
mod catalog_controls;
mod core;
mod transport;
mod ui;
pub use ui::leptos::ProductAdmin;
`,
    "crates/rustok-product/admin/src/catalog_controls.rs": `
pub(crate) struct ProductAdminListInput {
  pub search: Option<String>, pub status: Option<String>,
  pub category_id: Option<String>, pub sort_by: Option<String>,
  pub sort_direction: Option<String>,
}
pub(crate) fn build_product_admin_list_input() {}
pub(crate) fn build_product_admin_catalog_controls_labels() {}
`,
    "crates/rustok-product/admin/src/core.rs": `
pub(crate) enum ProductAdminOpenProductViewModel { Empty }
pub(crate) enum ProductAdminProductsLoadViewModel { Empty }
pub(crate) struct ProductAttributeEditorState;
pub(crate) fn build_save_command() {}
pub(crate) fn product_admin_selected_product_query_state() {}
pub(crate) fn product_admin_products_load_view_from_result() {}
`,
    "crates/rustok-product/admin/src/ui/leptos.rs": options.missingCategoryUi ? `
mod legacy;
use crate::catalog_controls::build_product_admin_catalog_controls_labels;
use crate::transport;
pub fn ProductAdmin() { let _ = transport::fetch_catalog_search_options; }
` : `
mod legacy;
use crate::catalog_controls::build_product_admin_catalog_controls_labels;
use crate::transport;
pub fn ProductAdmin() {
 let _ = read_route_query_value(&route_context, "category_id");
 let _ = view! { <form><select name="category_id"></select><select name="sort_by"></select><select name="sort_direction"></select><legacy::ProductAdmin /></form> };
 let _ = transport::fetch_catalog_search_options;
}
`,
    "crates/rustok-product/admin/src/ui/legacy_leptos.rs": options.missingLegacyEditor ? "pub fn ProductAdmin() {}" : `
pub fn ProductAdmin() {
 let _ = TypedProductAttributeField;
 let _ = build_save_command;
 let _ = product_admin_products_load_view_from_result;
 let _ = transport::fetch_products;
 let _ = save_product_attribute_values;
}
`,
    "crates/rustok-product/admin/src/transport.rs": `
mod admin_catalog_graphql;
mod admin_catalog_native;
pub(crate) use legacy::*;
pub async fn fetch_products() {
 build_product_admin_list_input();
 let _ = browser_query_value("category_id");
 let _ = browser_query_value("sort_by");
 let _ = browser_query_value("sort_direction");
 admin_catalog_native::fetch_products();
 admin_catalog_graphql::fetch_products();
}
`,
    "crates/rustok-product/admin/src/transport/legacy.rs": `
fn fetch_bootstrap() {} fn fetch_product() {} fn fetch_product_pricing() {}
fn fetch_catalog_categories() {} fn fetch_effective_product_form() {}
fn save_product_attribute_values() {} fn create_product() {}
fn update_product() {} fn delete_product() {}
`,
    "crates/rustok-product/admin/src/transport/admin_catalog_graphql.rs": `
use rustok_graphql::GraphqlRequest;
const QUERY: &str = "ProductAdminCatalog adminProductCatalog AdminProductCatalogFilter categoryId sortBy sortDirection primaryCategoryId";
`,
    "crates/rustok-product/admin/src/transport/admin_catalog_native.rs": options.missingNativeOwner ? `
#[server(prefix = "/api/fn", endpoint = "product/admin/catalog-list")]
fn endpoint() { let _ = HostRuntimeContext; }
` : `
#[server(prefix = "/api/fn", endpoint = "product/admin/catalog-list")]
fn endpoint() {
 let _ = HostRuntimeContext; let _ = TransactionalEventBus; let _ = PRODUCTS_LIST;
 let _ = AdminProductListQuery::try_from_transport;
 service.list_admin_products_with_query();
 let _ = primary_category_id;
}
`,
    "crates/rustok-product/src/services/catalog/types.rs": `
pub struct AdminProductListQuery {
 pub status: Option<String>, pub category_id: Option<String>,
 sort_by: String, sort_direction: String,
}
const STATUS_ERROR: &str = "status must be \\`draft\\`, \\`active\\`, or \\`archived\\`";
`,
    "crates/rustok-product/src/services/catalog/admin_queries.rs": options.missingOwnerStatus ? `
pub async fn list_admin_products_with_query() {
 let _ = TenantId.eq(tenant_id); let _ = PrimaryCategoryId.eq(category_id);
 let _ = admin_product_title_search_condition; order_by_asc(); order_by_desc();
 let _ = shipping_profile_slug; let _ = primary_category_id;
}
` : `
pub async fn list_admin_products_with_query() {
 let _ = TenantId.eq(tenant_id); let _ = Status.eq(status); let _ = PrimaryCategoryId.eq(category_id);
 let _ = admin_product_title_search_condition; order_by_asc(); order_by_desc();
 let _ = shipping_profile_slug; let _ = primary_category_id;
}
`,
    "crates/rustok-commerce/src/graphql/product_catalog.rs": `
struct AdminProductCatalogFilter;
async fn admin_product_catalog() {
 require_commerce_permission(); product_query_tenant();
 AdminProductListQuery::try_from_transport(); service.list_admin_products_with_query();
}
`,
    "crates/rustok-product/docs/implementation-plan.md": "verify-product-admin-boundary.mjs",
    "docs/modules/registry.md": "verify-product-admin-boundary.mjs",
    "package.json": JSON.stringify({ scripts: {
      "verify:product:admin-boundary": "node scripts/verify/verify-product-admin-boundary.mjs",
      "test:verify:product:admin-boundary": "node scripts/verify/verify-product-admin-boundary.test.mjs",
    }}),
  };
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-admin-boundary-"));
  for (const [relativePath, content] of Object.entries(fixtureSources(options))) {
    writeFixtureFile(root, relativePath, content);
  }
  return root;
}

function runVerifier(root) {
  return spawnSync("node", [scriptPath], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

function assertFixtureFails(options, pattern) {
  const root = withFixture(options);
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "expected mutated fixture to fail");
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("product admin boundary verifier passes composed canonical fixture", () => {
  const root = withFixture();
  try {
    const result = runVerifier(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /product admin boundary verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("product admin boundary rejects missing category UI", () => {
  assertFixtureFails({ missingCategoryUi: true }, /composed admin UI marker/);
});

test("product admin boundary rejects missing legacy editor", () => {
  assertFixtureFails({ missingLegacyEditor: true }, /preserved editor marker/);
});

test("product admin boundary rejects missing native owner execution", () => {
  assertFixtureFails({ missingNativeOwner: true }, /native admin list marker/);
});

test("product admin boundary rejects missing owner status filtering", () => {
  assertFixtureFails({ missingOwnerStatus: true }, /owner-side admin list execution marker/);
});
