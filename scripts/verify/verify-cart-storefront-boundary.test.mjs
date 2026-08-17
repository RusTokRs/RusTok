#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-cart-storefront-boundary.mjs");
const repoRoot = path.resolve(".");

function writeFixtureFile(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-cart-storefront-boundary-"));
  writeFixtureFile(root, "crates/rustok-cart/storefront/src/lib.rs", `
${options.legacyModApi ? "mod api;" : ""}
pub mod core;
pub mod model;
pub mod transport;
mod ui;
pub use ui::leptos::{CartCheckoutHandoffCard, CartView};
`);
  writeFixtureFile(root, "crates/rustok-cart/storefront/src/core/mod.rs", `
${options.includeLeptosCore ? "use leptos::prelude::*;" : ""}
pub struct CartFetchRequest;
pub struct CartLineItemDecrementRequest;
pub struct CartLineItemMutationRequest;
pub fn parse_cart_id() {}
pub fn parse_line_item_id() {}
`);
  writeFixtureFile(root, "crates/rustok-cart/storefront/src/model.rs", `
pub struct StorefrontCartLineItem {
  pub seller_id: Option<String>,
}
pub struct StorefrontCartShippingOption {
  pub id: String,
  pub name: String,
  pub currency_code: String,
  pub amount: String,
  pub provider_id: String,
  pub active: bool,
}
pub struct StorefrontCartDeliveryGroup {
  pub seller_id: Option<String>,
  pub available_shipping_options: Vec<StorefrontCartShippingOption>,
}
`);
  writeFixtureFile(root, "crates/rustok-cart/storefront/src/ui/leptos.rs", `
use crate::core;
use crate::transport;
pub fn CartView() {
  let _ = transport::fetch_cart;
  ${options.rawApiCall ? "let _ = api::fetch_storefront_cart_graphql;" : ""}
}
pub fn CartCheckoutHandoffCard() {}
`);
  writeFixtureFile(root, "crates/rustok-cart/storefront/src/transport/mod.rs", options.rawApiTransport ? "mod graphql_adapter;\nmod native_server_adapter;\npub async fn fetch_cart() {}\npub async fn decrement_line_item() {}\npub async fn remove_line_item() {}\nuse crate::api;\n" : readFileSync(path.join(repoRoot, "crates/rustok-cart/storefront/src/transport/mod.rs"), "utf8"));
  writeFixtureFile(root, "crates/rustok-cart/storefront/src/transport/graphql_adapter.rs", readFileSync(path.join(repoRoot, "crates/rustok-cart/storefront/src/transport/graphql_adapter.rs"), "utf8"));
  writeFixtureFile(root, "crates/rustok-cart/storefront/src/transport/graphql_error_safety.rs", readFileSync(path.join(repoRoot, "crates/rustok-cart/storefront/src/transport/graphql_error_safety.rs"), "utf8"));
  writeFixtureFile(root, "crates/rustok-cart/storefront/src/transport/native_server_adapter.rs", readFileSync(path.join(repoRoot, "crates/rustok-cart/storefront/src/transport/native_server_adapter.rs"), "utf8"));
  writeFixtureFile(root, "crates/rustok-cart/storefront/src/transport/native_server_adapter_ssr.rs", readFileSync(path.join(repoRoot, "crates/rustok-cart/storefront/src/transport/native_server_adapter_ssr.rs"), "utf8"));
  writeFixtureFile(root, "crates/rustok-cart/storefront/src/transport/native_server_mapping.rs", readFileSync(path.join(repoRoot, "crates/rustok-cart/storefront/src/transport/native_server_mapping.rs"), "utf8"));
  writeFixtureFile(root, "crates/rustok-cart/contracts/evidence/storefront-native-error-safety-source.json", readFileSync(path.join(repoRoot, "crates/rustok-cart/contracts/evidence/storefront-native-error-safety-source.json"), "utf8"));
  writeFixtureFile(root, "crates/rustok-cart/contracts/evidence/storefront-graphql-error-safety-source.json", readFileSync(path.join(repoRoot, "crates/rustok-cart/contracts/evidence/storefront-graphql-error-safety-source.json"), "utf8"));
  writeFixtureFile(root, "crates/rustok-cart/contracts/evidence/storefront-graphql-error-safety-source-review.json", readFileSync(path.join(repoRoot, "crates/rustok-cart/contracts/evidence/storefront-graphql-error-safety-source-review.json"), "utf8"));
  writeFixtureFile(root, "crates/rustok-cart/docs/storefront-native-error-safety.md", readFileSync(path.join(repoRoot, "crates/rustok-cart/docs/storefront-native-error-safety.md"), "utf8"));
  writeFixtureFile(root, "crates/rustok-cart/docs/storefront-graphql-error-safety.md", readFileSync(path.join(repoRoot, "crates/rustok-cart/docs/storefront-graphql-error-safety.md"), "utf8"));
  writeFixtureFile(root, "crates/rustok-commerce/docs/implementation-plan.md", readFileSync(path.join(repoRoot, "crates/rustok-commerce/docs/implementation-plan.md"), "utf8"));
  if (options.legacyApi) writeFixtureFile(root, "crates/rustok-cart/storefront/src/api.rs", "pub async fn fetch_storefront_cart_graphql() {}\n");
  writeFixtureFile(root, "crates/rustok-cart/docs/implementation-plan.md", "verify-cart-storefront-boundary.mjs");
  writeFixtureFile(root, "docs/modules/registry.md", "verify-cart-storefront-boundary.mjs");
  writeFixtureFile(root, "package.json", JSON.stringify({
    scripts: {
      "verify:cart:storefront-boundary": "node scripts/verify/verify-cart-storefront-boundary.mjs",
      "test:verify:cart:storefront-boundary": "node scripts/verify/verify-cart-storefront-boundary.test.mjs",
      "test:verify:ffa:ui:migration": "npm run test:verify:cart:storefront-boundary",
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

function withTempFixture(options, assertion) {
  const root = withFixture(options);
  try {
    assertion(runVerifier(root));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("cart storefront boundary verifier passes canonical fixture", () => {
  withTempFixture({}, (result) => {
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /cart storefront boundary verification passed/);
  });
});

test("cart storefront boundary verifier rejects legacy api file", () => {
  withTempFixture({ legacyApi: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /legacy api\.rs/);
  });
});

test("cart storefront boundary verifier rejects legacy api module wiring", () => {
  withTempFixture({ legacyModApi: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must not wire legacy api adapter/);
  });
});

test("cart storefront boundary verifier rejects Leptos-specific core", () => {
  withTempFixture({ includeLeptosCore: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /core must stay Leptos\/server-function free/);
  });
});

test("cart storefront boundary verifier rejects raw api calls from UI", () => {
  withTempFixture({ rawApiCall: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /UI adapter must not call raw transport or services/);
  });
});
