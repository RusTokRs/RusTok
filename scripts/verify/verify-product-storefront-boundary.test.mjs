#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-product-storefront-boundary.mjs");

function writeFixtureFile(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function libSource() {
  return `
mod catalog_controls;
mod core;
mod transport;
mod ui;

pub use ui::leptos::ProductView;
`;
}

function catalogControlsSource() {
  return `
use rustok_ui_core::normalize_optional_ui_text;
pub struct CatalogListInput { pub search: Option<String> }
pub fn build_catalog_search_labels() {}
`;
}

function coreSource({ includeLeptos = false, omitCatalogLabels = false } = {}) {
  return `
${includeLeptos ? "use leptos::prelude::*;" : ""}
${omitCatalogLabels ? "" : "pub fn build_product_catalog_rail_labels() {}"}
pub fn build_catalog_rail_view_model() {}
pub fn build_shell_view_model() {}
pub fn build_transport_error_dom_evidence() {}
pub fn build_selected_product_empty_view_model() {}
pub fn build_selected_product_view_model() {}
pub fn build_fetch_request() {}
pub fn build_route_input() {}
pub fn resolve_route_segment() {}
pub struct ProductCatalogRailViewModel { pub show_empty_state: bool }
pub struct SelectedProductViewModel { pub metadata_items: Vec<String> }
`;
}

function uiSource({
  rawApiCall = false,
  directCatalogLabels = false,
  metadataSeparator = false,
  routeSegmentFallback = false,
  catalogEmptyBranch = false,
  omitSearchControl = false,
} = {}) {
  return `
use crate::catalog_controls::build_catalog_list_input;
use crate::core::{build_product_catalog_rail_labels, build_catalog_rail_view_model, resolve_route_segment};
use crate::transport;

pub fn ProductView() {
    ${omitSearchControl ? "" : 'let controls = build_catalog_list_input(read_route_query_value(&route_context, "search"));'}
    ${omitSearchControl ? "" : 'let _search = view! { <input name="search" /> };'}
    ${omitSearchControl ? "" : "let _transport = transport::fetch_products(request, controls);"}
    let _labels = build_product_catalog_rail_labels;
    let _rail = build_catalog_rail_view_model;
    let _route_segment = resolve_route_segment;
    ${rawApiCall ? "let _raw = api::fetch_products;" : ""}
    ${directCatalogLabels ? 'let _copy = "Published products";' : ""}
    ${metadataSeparator ? 'let _separator = view! { <span>"|"</span> };' : ""}
    ${routeSegmentFallback ? 'let _fallback = route_segment.unwrap_or_else(|| "products".to_string());' : ""}
    ${catalogEmptyBranch ? "if view_model.items.is_empty() {}" : ""}
}
`;
}

function transportSource() {
  return `
mod catalog_list_native;
mod graphql_adapter;
mod native_server_adapter;
use crate::catalog_controls::CatalogListInput;
pub async fn fetch_products(request: FetchRequest, controls: CatalogListInput) {
    catalog_list_native::fetch_products(request, controls);
}
`;
}

function catalogListNativeSource({ omitNativeSearch = false } = {}) {
  return `
#[server(prefix = "/api/fn", endpoint = "product/storefront/catalog-list")]
async fn storefront_catalog_list_native(search: Option<String>) {
  ${omitNativeSearch ? "" : "let query = StorefrontProductListQuery::try_from_transport(search, category_id, sort_by, sort_direction);"}
  ${omitNativeSearch ? "" : "service.list_published_products_with_query(query);"}
}
`;
}

function graphqlAdapterSource({ omitGraphqlSearch = false } = {}) {
  return `
use rustok_graphql::GraphqlRequest;
pub async fn fetch_storefront_products(controls: CatalogListInput) {
  let filter = StorefrontProductsFilter {
    ${omitGraphqlSearch ? "search: None," : "search: controls.search,"}
  };
}
`;
}

function nativeServerAdapterSource() {
  return `
use rustok_api::HostRuntimeContext;
use rustok_outbox::TransactionalEventBus;
#[server(prefix = "/api/fn", endpoint = "product/storefront-data")]
async fn storefront_products_native() {
  let runtime_ctx = expect_context::<HostRuntimeContext>();
  let event_bus = runtime_ctx.shared_get::<TransactionalEventBus>();
  let db = runtime_ctx.db_clone();
}
`;
}

function catalogQueriesSource() {
  return `
pub async fn list_published_products_with_query() {
  let condition = product_title_search_condition(backend, search);
}
fn product_title_search_condition() {}
`;
}

const repoRoot = path.resolve(".");

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-storefront-boundary-"));
  writeFixtureFile(root, "crates/rustok-product/storefront/src/lib.rs", libSource());
  writeFixtureFile(root, "crates/rustok-product/storefront/src/catalog_controls.rs", catalogControlsSource());
  writeFixtureFile(root, "crates/rustok-product/storefront/src/core.rs", coreSource(options));
  writeFixtureFile(root, "crates/rustok-product/storefront/src/ui/leptos.rs", uiSource(options));
  writeFixtureFile(root, "crates/rustok-product/storefront/src/transport/mod.rs", readFileSync(path.join(repoRoot, "crates/rustok-product/storefront/src/transport/mod.rs"), "utf8"));
  const realCatalogList = readFileSync(path.join(repoRoot, "crates/rustok-product/storefront/src/transport/catalog_list_native.rs"), "utf8");
  writeFixtureFile(
    root,
    "crates/rustok-product/storefront/src/transport/catalog_list_native.rs",
    options.omitNativeSearch
      ? realCatalogList.replace(".list_published_products_with_query(", ".list_published_products(")
      : realCatalogList,
  );
  const realGqlAdapter = readFileSync(path.join(repoRoot, "crates/rustok-product/storefront/src/transport/graphql_adapter.rs"), "utf8");
  writeFixtureFile(
    root,
    "crates/rustok-product/storefront/src/transport/graphql_adapter.rs",
    options.omitGraphqlSearch
      ? realGqlAdapter.replace("search: controls.search,", "search: None,")
      : realGqlAdapter,
  );
  writeFixtureFile(root, "crates/rustok-product/storefront/src/transport/graphql_error_safety.rs", readFileSync(path.join(repoRoot, "crates/rustok-product/storefront/src/transport/graphql_error_safety.rs"), "utf8"));
  writeFixtureFile(root, "crates/rustok-product/storefront/src/transport/native_server_adapter.rs", readFileSync(path.join(repoRoot, "crates/rustok-product/storefront/src/transport/native_server_adapter.rs"), "utf8"));
  writeFixtureFile(root, "crates/rustok-product/src/services/catalog/queries.rs", catalogQueriesSource());
  writeFixtureFile(root, "crates/rustok-product/storefront/Cargo.toml", readFileSync(path.join(repoRoot, "crates/rustok-product/storefront/Cargo.toml"), "utf8"));
  writeFixtureFile(root, "crates/rustok-product/contracts/evidence/storefront-catalog-native-error-safety-source.json", readFileSync(path.join(repoRoot, "crates/rustok-product/contracts/evidence/storefront-catalog-native-error-safety-source.json"), "utf8"));
  writeFixtureFile(root, "crates/rustok-product/contracts/evidence/storefront-graphql-error-safety-source.json", readFileSync(path.join(repoRoot, "crates/rustok-product/contracts/evidence/storefront-graphql-error-safety-source.json"), "utf8"));
  writeFixtureFile(root, "crates/rustok-product/contracts/evidence/storefront-graphql-error-safety-source-review.json", readFileSync(path.join(repoRoot, "crates/rustok-product/contracts/evidence/storefront-graphql-error-safety-source-review.json"), "utf8"));
  writeFixtureFile(root, "crates/rustok-product/docs/storefront-graphql-error-safety.md", readFileSync(path.join(repoRoot, "crates/rustok-product/docs/storefront-graphql-error-safety.md"), "utf8"));
  if (options.legacyApi) writeFixtureFile(root, "crates/rustok-product/storefront/src/api.rs", nativeServerAdapterSource());
  writeFixtureFile(root, "crates/rustok-product/docs/implementation-plan.md", "verify-product-storefront-boundary.mjs");
  writeFixtureFile(root, "docs/modules/registry.md", "verify-product-storefront-boundary.mjs");
  writeFixtureFile(root, "package.json", JSON.stringify({
    scripts: {
      "verify:product:storefront-boundary": "node scripts/verify/verify-product-storefront-boundary.mjs",
      "test:verify:product:storefront-boundary": "node scripts/verify/verify-product-storefront-boundary.test.mjs",
      "test:verify:ffa:ui:migration": "npm run test:verify:product:storefront-boundary",
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

function assertFixtureFails(options, pattern, message) {
  const root = withFixture(options);
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, message);
    assert.match(result.stderr, pattern);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("product storefront boundary verifier passes canonical fixture", () => {
  const root = withFixture();
  try {
    const result = runVerifier(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /product storefront boundary verification passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("product storefront boundary verifier rejects missing search control", () => {
  assertFixtureFails(
    { omitSearchControl: true },
    /snake_case search query key|search query control|typed controls/,
    "Expected missing storefront search control fixture to fail",
  );
});

test("product storefront boundary verifier rejects missing GraphQL search mapping", () => {
  assertFixtureFails(
    { omitGraphqlSearch: true },
    /GraphQL storefront list must carry typed search state/,
    "Expected missing GraphQL search mapping fixture to fail",
  );
});

test("product storefront boundary verifier rejects missing native owner search", () => {
  assertFixtureFails(
    { omitNativeSearch: true },
    /native catalog list must validate typed controls|execute the owner service query/,
    "Expected missing native owner search fixture to fail",
  );
});

test("product storefront boundary verifier rejects Leptos-specific core", () => {
  assertFixtureFails(
    { includeLeptos: true },
    /core must stay Leptos\/server-function free/,
    "Expected Leptos core fixture to fail",
  );
});

test("product storefront boundary verifier rejects missing catalog labels helper", () => {
  assertFixtureFails(
    { omitCatalogLabels: true },
    /build_product_catalog_rail_labels/,
    "Expected missing catalog labels fixture to fail",
  );
});

test("product storefront boundary verifier rejects catalog copy in UI", () => {
  assertFixtureFails(
    { directCatalogLabels: true },
    /catalog rail copy\/label policy must stay in core/,
    "Expected direct catalog copy fixture to fail",
  );
});

test("product storefront boundary verifier rejects selected metadata separators in UI", () => {
  assertFixtureFails(
    { metadataSeparator: true },
    /selected-product metadata display policy must stay in core/,
    "Expected direct metadata separator fixture to fail",
  );
});

test("product storefront boundary verifier rejects route segment fallback in UI", () => {
  assertFixtureFails(
    { routeSegmentFallback: true },
    /route segment fallback policy must stay in core/,
    "Expected route segment fallback fixture to fail",
  );
});

test("product storefront boundary verifier rejects catalog empty-state policy in UI", () => {
  assertFixtureFails(
    { catalogEmptyBranch: true },
    /catalog rail empty-state policy must stay in core/,
    "Expected catalog empty-state fixture to fail",
  );
});

test("product storefront boundary verifier rejects raw api calls from UI", () => {
  assertFixtureFails(
    { rawApiCall: true },
    /UI adapter must not call raw transport or services/,
    "Expected raw UI api fixture to fail",
  );
});

test("product storefront boundary verifier rejects legacy api module", () => {
  assertFixtureFails(
    { legacyApi: true },
    /legacy api\.rs/,
    "Expected legacy api fixture to fail",
  );
});
