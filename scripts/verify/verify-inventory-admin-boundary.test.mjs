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

function transportFacadeSource() {
  return `
pub async fn fetch_bootstrap() { native_server_adapter::fetch_bootstrap().await; }
pub async fn fetch_products() { native_server_adapter::fetch_products().await; }
pub async fn fetch_product() { native_server_adapter::fetch_product().await; }
pub async fn set_variant_quantity() { native_server_adapter::set_variant_quantity().await; }
pub async fn adjust_variant_quantity() { native_server_adapter::adjust_variant_quantity().await; }
pub async fn reserve_variant_quantity() { native_server_adapter::reserve_variant_quantity().await; }
pub async fn release_reservation_quantity() { native_server_adapter::release_reservation_quantity().await; }
pub async fn check_variant_availability() { native_server_adapter::check_variant_availability().await; }
`;
}

function nativeAdapterSource({ includeWriteFallback = false } = {}) {
  if (includeWriteFallback) {
    return `
pub async fn set_variant_quantity(token: Option<String>, tenant_slug: Option<String>) {
    graphql_selected_path_products(token, tenant_slug, "tenant".to_string(), None, None, None).await;
    removed_graphql_read_transport().fetch_products().await;
    CommerceGraphqlInventoryReadAdapter;
}
pub async fn adjust_variant_quantity() {}
pub async fn reserve_variant_quantity() {}
pub async fn release_reservation_quantity() {}
pub async fn check_variant_availability() {}
pub async fn fetch_bootstrap() {}
pub async fn fetch_products() {}
pub async fn fetch_product() {}
`;
  }

  return `
fn inventory_read_service_from_context() {
    let runtime_ctx = leptos::prelude::expect_context::<rustok_api::HostRuntimeContext>();
    runtime_ctx.db_clone();
}
fn inventory_service_from_context() {
    let runtime_ctx = leptos::prelude::expect_context::<rustok_api::HostRuntimeContext>();
    runtime_ctx.db_clone();
    runtime_ctx.shared_get::<rustok_outbox::TransactionalEventBus>();
}
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
pub async fn fetch_bootstrap() { inventory_bootstrap_native().await; }
pub async fn fetch_products() { inventory_products_native().await; }
pub async fn fetch_product() { inventory_product_native().await; }
pub async fn set_variant_quantity() { inventory_set_quantity_native().await; }
pub async fn adjust_variant_quantity() { inventory_adjust_quantity_native().await; }
pub async fn reserve_variant_quantity() { inventory_reserve_quantity_native().await; }
pub async fn release_reservation_quantity() { inventory_release_reservation_native().await; }
pub async fn check_variant_availability() { inventory_check_availability_native().await; }
`;
}

function transportSource() {
  return `
use rustok_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
pub struct CommerceGraphqlInventoryReadAdapter;
const BOOTSTRAP_QUERY: &str = "query Bootstrap /api/graphql RUSTOK_GRAPHQL_URL";
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
use rustok_inventory::{load_inventory_projection_by_variant_for_public_channel, PublicChannelInventoryVariantProjectionInput};
async fn apply_public_channel_inventory_to_product() {
    let input = PublicChannelInventoryVariantProjectionInput { variant_id, inventory_policy: policy.as_str() };
    let projections = load_inventory_projection_by_variant_for_public_channel().await;
}
`;
}

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-inventory-boundary-"));
  writeFixtureFile(root, "crates/rustok-inventory/src/services/inventory.rs", inventorySource(options));
  writeFixtureFile(root, "crates/rustok-inventory/admin/src/transport/mod.rs", transportFacadeSource());
  writeFixtureFile(root, "crates/rustok-inventory/admin/src/transport/native_server_adapter.rs", nativeAdapterSource(options));
  if (options.includeTransportFile) {
    writeFixtureFile(root, "crates/rustok-inventory/admin/src/transport.rs", transportSource());
  }
  if (options.includeLegacyApiFile) {
    writeFixtureFile(root, "crates/rustok-inventory/admin/src/api.rs", transportFacadeSource());
  }
  writeFixtureFile(root, "crates/rustok-inventory/admin/src/lib.rs", "mod core;\nmod transport;\n");
  writeFixtureFile(root, "crates/rustok-inventory/admin/src/core.rs", "");
  writeFixtureFile(root, "crates/rustok-inventory/admin/src/model.rs", "");
  writeFixtureFile(root, "crates/rustok-inventory/admin/src/ui/leptos.rs", "native inventory facade");
  writeFixtureFile(root, "crates/rustok-inventory/admin/locales/en.json", "{\"inventory.subtitle\":\"native inventory facade\"}\n");
  writeFixtureFile(root, "crates/rustok-inventory/admin/locales/ru.json", "{\"inventory.subtitle\":\"native inventory facade\"}\n");
  writeFixtureFile(root, "crates/rustok-inventory/admin/Cargo.toml", "[package]\nname = \"rustok-inventory-admin\"\n");
  writeFixtureFile(root, "crates/rustok-inventory/docs/implementation-plan.md", "- Next step: verification/CI evidence slice for inventory boundary.\n- [x] move current inventory admin UI stock operations to inventory-owned native/transport mutations\n");
  for (const relativePath of [
    "crates/rustok-commerce/src/graphql/mutations/helpers.rs",
    "crates/rustok-commerce/src/services/checkout.rs",
    "crates/rustok-commerce/src/controllers/store/mod.rs",
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
    assert.match(result.stderr, /removed GraphQL adapter file must stay absent/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});


test("inventory admin boundary verifier rejects leftover pre-FFA api facade", () => {
  const root = withFixture({ includeLegacyApiFile: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected leftover api fixture to fail");
    assert.match(result.stderr, /remove the pre-FFA api facade/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("inventory admin boundary verifier rejects removed GraphQL write fallback", () => {
  const root = withFixture({ includeWriteFallback: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected removed GraphQL write fallback fixture to fail");
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
