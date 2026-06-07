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

function apiSource() {
  return `
pub async fn set_variant_quantity() { crate::native::set_variant_quantity().await; }
pub async fn adjust_variant_quantity() { crate::native::adjust_variant_quantity().await; }
pub async fn reserve_variant_quantity() { crate::native::reserve_variant_quantity().await; }
pub async fn release_reservation_quantity() { crate::native::release_reservation_quantity().await; }
pub async fn check_variant_availability() { crate::native::check_variant_availability().await; }
`;
}

function transportSource({ includeMutation = false } = {}) {
  return `
const BOOTSTRAP_QUERY: &str = "query Bootstrap";
const PRODUCTS_QUERY: &str = "query Products";
const PRODUCT_QUERY: &str = "query Product";
${includeMutation ? 'const BAD_MUTATION: &str = "mutation setVariantQuantity";' : ''}
`;
}

function nativeSource() {
  return `
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

function withFixture(options = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "rustok-inventory-boundary-"));
  writeFixtureFile(root, "crates/rustok-inventory/src/services/inventory.rs", inventorySource(options));
  writeFixtureFile(root, "crates/rustok-inventory/admin/src/api.rs", apiSource());
  writeFixtureFile(root, "crates/rustok-inventory/admin/src/transport.rs", transportSource(options));
  writeFixtureFile(root, "crates/rustok-inventory/admin/src/native.rs", nativeSource());
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

test("inventory admin boundary verifier rejects GraphQL mutation drift", () => {
  const root = withFixture({ includeMutation: true });
  try {
    const result = runVerifier(root);
    assert.notEqual(result.status, 0, "Expected GraphQL mutation fixture to fail");
    assert.match(result.stderr, /transitional GraphQL adapter must remain read-only/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
