#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
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
mod core;
mod transport;
mod ui;

pub use ui::leptos::ProductView;
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
} = {}) {
  return `
use crate::core::{build_product_catalog_rail_labels, build_catalog_rail_view_model, resolve_route_segment};
use crate::transport;

pub fn ProductView() {
    let _transport = transport::fetch_products;
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
mod graphql_adapter;
mod native_server_adapter;
pub async fn fetch_products() {}
`;
}

function graphqlAdapterSource() {
  return `
use rustok_graphql::GraphqlRequest;
pub async fn fetch_storefront_products() {}
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

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-product-storefront-boundary-"));
  writeFixtureFile(root, "crates/rustok-product/storefront/src/lib.rs", libSource());
  writeFixtureFile(root, "crates/rustok-product/storefront/src/core.rs", coreSource(options));
  writeFixtureFile(root, "crates/rustok-product/storefront/src/ui/leptos.rs", uiSource(options));
  writeFixtureFile(root, "crates/rustok-product/storefront/src/transport/mod.rs", transportSource());
  writeFixtureFile(root, "crates/rustok-product/storefront/src/transport/graphql_adapter.rs", graphqlAdapterSource());
  writeFixtureFile(root, "crates/rustok-product/storefront/src/transport/native_server_adapter.rs", nativeServerAdapterSource());
  writeFixtureFile(root, "crates/rustok-product/storefront/Cargo.toml", "[package]\nname = \"rustok-product-storefront\"\n");
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

test("product storefront boundary verifier rejects Leptos-specific core", () => {
  const root = withFixture({ includeLeptos: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected Leptos core fixture to fail");
    assert.match(result.stderr, /core must stay Leptos\/server-function free/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("product storefront boundary verifier rejects missing catalog labels helper", () => {
  const root = withFixture({ omitCatalogLabels: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected missing catalog labels fixture to fail");
    assert.match(result.stderr, /build_product_catalog_rail_labels/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("product storefront boundary verifier rejects catalog copy in UI", () => {
  const root = withFixture({ directCatalogLabels: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected direct catalog copy fixture to fail");
    assert.match(result.stderr, /catalog rail copy\/label policy must stay in core/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("product storefront boundary verifier rejects selected metadata separators in UI", () => {
  const root = withFixture({ metadataSeparator: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected direct metadata separator fixture to fail");
    assert.match(result.stderr, /selected-product metadata display policy must stay in core/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("product storefront boundary verifier rejects route segment fallback in UI", () => {
  const root = withFixture({ routeSegmentFallback: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected route segment fallback fixture to fail");
    assert.match(result.stderr, /route segment fallback policy must stay in core/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("product storefront boundary verifier rejects catalog empty-state policy in UI", () => {
  const root = withFixture({ catalogEmptyBranch: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected catalog empty-state fixture to fail");
    assert.match(result.stderr, /catalog rail empty-state policy must stay in core/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("product storefront boundary verifier rejects raw api calls from UI", () => {
  const root = withFixture({ rawApiCall: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected raw UI api fixture to fail");
    assert.match(result.stderr, /UI adapter must not call raw transport or services/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("product storefront boundary verifier rejects legacy api module", () => {
  const root = withFixture({ legacyApi: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected legacy api fixture to fail");
    assert.match(result.stderr, /legacy api\.rs/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
