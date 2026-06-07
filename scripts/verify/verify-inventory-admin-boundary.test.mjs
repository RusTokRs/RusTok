#!/usr/bin/env node

import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const scriptPath = path.resolve("scripts/verify/verify-inventory-admin-boundary.mjs");

function writeFixtureFile(root, relativePath, content) {
  const filePath = path.join(root, relativePath);
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, content);
}

function inventorySource({ includePreRead = false } = {}) {
  return `
fn inventory_policy_allows_backorder(inventory_policy: &str) -> bool {
    inventory_policy.eq_ignore_ascii_case("continue")
}

struct InventoryQuantityWriteResult {
    quantity: i32,
    in_stock: bool,
}

impl InventoryQuantityWriteResult {
    fn from_quantity_and_policy(quantity: i32, inventory_policy: &str) -> Self {
        Self {
            quantity,
            in_stock: quantity > 0 || inventory_policy_allows_backorder(inventory_policy),
        }
    }
}

impl InventoryService {
    pub async fn adjust_variant_quantity(&self) -> Result<InventoryQuantityWriteResult, String> {
        let update = self.adjust_inventory_update().await?;
        Ok(InventoryQuantityWriteResult::from_quantity_and_policy(
            update.quantity,
            &update.inventory_policy,
        ))
    }

    async fn adjust_inventory_update(&self) -> Result<InventoryQuantityUpdate, String> {
        let variant = Variant { inventory_policy: "continue".to_string() };
        let inventory_policy = variant.inventory_policy.clone();
        Ok(InventoryQuantityUpdate {
            quantity: 0,
            inventory_policy,
        })
    }

    pub async fn set_variant_quantity(&self) -> Result<InventoryQuantityWriteResult, String> {
        ${includePreRead ? 'let _variant = self.load_variant(&self.db).await?;' : ''}
        let update = self.set_inventory_update().await?;
        Ok(InventoryQuantityWriteResult::from_quantity_and_policy(
            update.quantity,
            &update.inventory_policy,
        ))
    }

    async fn set_inventory_update(&self) -> Result<InventoryQuantityUpdate, String> {
        let variant = Variant { inventory_policy: "continue".to_string() };
        let inventory_policy = variant.inventory_policy.clone();
        Ok(InventoryQuantityUpdate {
            quantity: 0,
            inventory_policy,
        })
    }
}

#[test]
fn quantity_write_result_honors_backorder_policy_for_native_write_facades() {}

struct InventoryService;
struct Variant { inventory_policy: String }
struct InventoryQuantityUpdate {
    quantity: i32,
    inventory_policy: String,
}
`;
}

function apiSource({ includeWriteFallback = false } = {}) {
  if (includeWriteFallback) {
    return `
pub async fn set_variant_quantity(token: Option<String>, tenant_slug: Option<String>) {
    fallback_products(token, tenant_slug, "tenant".to_string(), None, None, None).await;
    transitional_read_transport().fetch_products().await;
    CommerceGraphqlInventoryReadAdapter;
}
pub async fn adjust_variant_quantity() { crate::native::adjust_variant_quantity().await; }
pub async fn reserve_variant_quantity() { crate::native::reserve_variant_quantity().await; }
pub async fn release_reservation_quantity() { crate::native::release_reservation_quantity().await; }
pub async fn check_variant_availability() { crate::native::check_variant_availability().await; }
`;
  }

  return `
pub async fn fetch_bootstrap() { crate::native::fetch_bootstrap().await; }
pub async fn fetch_products() { crate::native::fetch_products().await; }
pub async fn fetch_product() { crate::native::fetch_product().await; }
pub async fn set_variant_quantity() { crate::native::set_variant_quantity().await; }
pub async fn adjust_variant_quantity() { crate::native::adjust_variant_quantity().await; }
pub async fn reserve_variant_quantity() { crate::native::reserve_variant_quantity().await; }
pub async fn release_reservation_quantity() { crate::native::release_reservation_quantity().await; }
pub async fn check_variant_availability() { crate::native::check_variant_availability().await; }
`;
}

function transportSource() {
  return `
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
pub struct CommerceGraphqlInventoryReadAdapter;
const BOOTSTRAP_QUERY: &str = "query Bootstrap /api/graphql RUSTOK_GRAPHQL_URL";
`;
}

function nativeSource() {
  return `
#[server(prefix = "/api/fn", endpoint = "inventory/bootstrap")]
async fn inventory_bootstrap_native() {}
#[server(prefix = "/api/fn", endpoint = "inventory/products")]
async fn inventory_products_native() {}
#[server(prefix = "/api/fn", endpoint = "inventory/product")]
async fn inventory_product_native() {}
#[server(prefix = "/api/fn", endpoint = "inventory/variant/set-quantity")]
async fn inventory_set_quantity_native() {}
#[server(prefix = "/api/fn", endpoint = "inventory/variant/adjust-quantity")]
async fn inventory_adjust_quantity_native() {}
#[server(prefix = "/api/fn", endpoint = "inventory/variant/reserve-quantity")]
async fn inventory_reserve_quantity_native() {}
#[server(prefix = "/api/fn", endpoint = "inventory/variant/release-reservation")]
async fn inventory_release_reservation_native() {}
#[server(prefix = "/api/fn", endpoint = "inventory/variant/check-availability")]
async fn inventory_check_availability_native() {}
`;
}

function commerceAvailabilityCallerSource({ includeDirectLookup = false } = {}) {
  if (includeDirectLookup) {
    return `
use rustok_inventory::inventory_policy_allows_backorder;
use crate::storefront_channel::load_available_inventory_for_variant_in_public_channel;
async fn validate_storefront_variant_inventory() {
    if inventory_policy_allows_backorder("continue") { return; }
    load_available_inventory_for_variant_in_public_channel().await;
}
`;
  }

  return `
use rustok_inventory::check_variant_availability_for_public_channel;
async fn validate_storefront_variant_inventory() {
    check_variant_availability_for_public_channel().await;
}
`;
}

function commerceStorefrontChannelSource({ includeDirectProjection = false } = {}) {
  if (includeDirectProjection) {
    return `
use rustok_inventory::{inventory_policy_allows_backorder, load_available_inventory_by_variant_for_public_channel};
async fn apply_public_channel_inventory_to_product() {
    let available_inventory = load_available_inventory_by_variant_for_public_channel().await;
    let in_stock = available_inventory > 0 || inventory_policy_allows_backorder("continue");
}
`;
  }

  return `
use rustok_inventory::load_inventory_projection_by_variant_for_public_channel;
async fn apply_public_channel_inventory_to_product() {
    let projections = load_inventory_projection_by_variant_for_public_channel().await;
}
`;
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-inventory-boundary-"));
  writeFixtureFile(root, "crates/rustok-inventory/src/services/inventory.rs", inventorySource(options));
  writeFixtureFile(root, "crates/rustok-inventory/admin/src/api.rs", apiSource(options));
  if (options.includeTransportFile) {
    writeFixtureFile(root, "crates/rustok-inventory/admin/src/transport.rs", transportSource());
  }
  writeFixtureFile(root, "crates/rustok-inventory/admin/src/native.rs", nativeSource());
  writeFixtureFile(root, "crates/rustok-inventory/admin/src/lib.rs", "mod api;\nmod core;\nmod native;\n");
  writeFixtureFile(root, "crates/rustok-inventory/admin/src/core.rs", "");
  writeFixtureFile(root, "crates/rustok-inventory/admin/src/model.rs", "");
  writeFixtureFile(root, "crates/rustok-inventory/admin/src/ui/leptos.rs", "");
  writeFixtureFile(root, "crates/rustok-inventory/admin/Cargo.toml", "[package]\nname = \"rustok-inventory-admin\"\n");
  for (const relativePath of [
    "crates/rustok-commerce/src/graphql/mutation.rs",
    "crates/rustok-commerce/src/services/checkout.rs",
    "crates/rustok-commerce/src/controllers/store.rs",
  ]) {
    writeFixtureFile(root, relativePath, commerceAvailabilityCallerSource(options));
  }
  writeFixtureFile(root, "crates/rustok-commerce/src/storefront_channel.rs", commerceStorefrontChannelSource(options));
  return root;
}

function runVerifier(root) {
  return spawnSync("node", [scriptPath], {
    cwd: path.resolve("."),
    env: { ...process.env, RUSTOK_VERIFY_REPO_ROOT: root },
    encoding: "utf8",
  });
}

test("inventory admin boundary verifier passes canonical fixture", () => {
  const root = withFixture();
  try {
    const result = runVerifier(root);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /Inventory admin boundary invariants passed/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("inventory admin boundary verifier rejects duplicate variant pre-read", () => {
  const root = withFixture({ includePreRead: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected duplicate pre-read fixture to fail");
    assert.match(result.stderr, /must not pre-read variant policy/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("inventory admin boundary verifier rejects leftover GraphQL transport file", () => {
  const root = withFixture({ includeTransportFile: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected leftover transport fixture to fail");
    assert.match(result.stderr, /remove the transitional GraphQL adapter file/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("inventory admin boundary verifier rejects transitional write fallback", () => {
  const root = withFixture({ includeWriteFallback: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected transitional write fallback fixture to fail");
    assert.match(result.stderr, /removed GraphQL fallback marker must stay absent/);
    assert.match(result.stderr, /must not accept auth tokens for a GraphQL fallback path/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});


test("inventory admin boundary verifier rejects direct commerce public-channel availability lookup", () => {
  const root = withFixture({ includeDirectLookup: true, includeDirectProjection: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected direct commerce availability lookup fixture to fail");
    assert.match(result.stderr, /must not call channel-visible inventory loaders directly/);
    assert.match(result.stderr, /must not duplicate backorder policy branching/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
