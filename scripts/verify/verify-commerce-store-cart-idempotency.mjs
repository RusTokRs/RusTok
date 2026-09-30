#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root =
  process.env.RUSTOK_VERIFY_REPO_ROOT?.trim() ||
  path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

const read = (p) => readFileSync(path.join(root, p), "utf8");
const failures = [];

const need = (s, v, l) => {
  if (!s.includes(v)) failures.push(l + ": missing " + v);
};
const forbid = (s, v, l) => {
  if (s.includes(v)) failures.push(l + ": forbidden " + v);
};

const carts = read("crates/modules/rustok-commerce/src/controllers/store/carts.rs");
const storeMod = read("crates/modules/rustok-commerce/src/controllers/store/mod.rs");
const shipping = read(
  "crates/modules/rustok-commerce/src/controllers/store/carts/shipping_owner_reads.rs",
);
const checkout = read("crates/modules/rustok-commerce/src/controllers/store/checkout.rs");
const cartService = read("crates/modules/rustok-cart/src/services/cart.rs");
const cartPorts = read("crates/modules/rustok-cart/src/ports.rs");
const cartCargo = read("crates/modules/rustok-cart/Cargo.toml");

for (const value of [
  "headers: HeaderMap,",
  "required_storefront_idempotency_key(&headers)?",
  "Some(&idempotency_key)",
  "Idempotency-Key",
]) need(carts, value, "Store Cart caller-owned idempotency");

for (const value of [
  "create_cart",
  "update_cart_context",
  "add_cart_line_item",
  "update_cart_line_item",
  "remove_cart_line_item",
]) {
  need(carts, "pub async fn " + value, value + " mounted write");
}

for (const value of [
  '"create",\n                Some(&idempotency_key)',
  '"update-context",\n                Some(&idempotency_key)',
  '"add-line-item",\n                Some(&idempotency_key)',
  '"update-line-item",\n                    Some(&idempotency_key)',
  '"remove-line-item",\n                Some(&idempotency_key)',
]) need(carts, value, "Cart write context");

for (const value of [
  '"read",\n                None',
  '"read",\n                    None',
]) need(carts, value, "Cart read context remains observational");

for (const value of [
  "pub(crate) fn storefront_cart_port_context(",
  "idempotency_key: Option<&str>",
  "Some(value) => context.with_idempotency_key(value.to_string())",
  "pub(crate) fn required_storefront_idempotency_key(",
  "MAX_LENGTH: usize = 191",
]) need(storeMod, value, "shared Store Cart context helper");

for (const value of [
  "with_idempotency_key(correlation_id)",
  "is_write: bool",
]) forbid(storeMod, value, "synthetic Store Cart idempotency");

need(shipping, "operation,\n        None,", "shipping owner read context");
need(shipping, "idempotency_key: &str", "shipping cart write key");
need(shipping, '"update-context",\n                Some(idempotency_key)', "shipping cart update key");
need(shipping, "idempotency_key,\n        storefront_port.as_ref(),", "shipping reprice key propagation");

need(checkout, "headers: HeaderMap,", "payment collection headers");
need(checkout, "let idempotency_key = required_idempotency_key(&headers)?;", "payment collection caller key");
need(checkout, "&idempotency_key,\n        cart_storefront_port.as_ref(),", "payment collection cart reprice key");
need(checkout, '"create_or_reuse_collection",\n        Some(&idempotency_key)', "payment collection owner key");
need(
  checkout,
  '("Idempotency-Key" = String, Header, description = "Caller-owned idempotency key")',
  "payment collection OpenAPI key",
);
for (const value of [
  'context.with_idempotency_key(format!("storefront-payment-collection:{cart_id}"))',
  "is_write: bool",
]) forbid(checkout, value, "synthetic payment collection idempotency");

const writeBodies = carts.match(/Some\(&idempotency_key\)/g) ?? [];
if (writeBodies.length !== 5) {
  failures.push("expected five direct Store Cart write key bindings, found " + writeBodies.length);
}

for (const value of [
  "rustok-outbox.workspace = true",
]) need(cartCargo, value, "Cart shared receipt dependency");

for (const value of [
  "pub(crate) async fn run_storefront_idempotent_write<T, F>(",
  "OwnerOperationScope::Tenant(tenant_id)",
  '"cart",',
  "idempotency::admit(",
  "Admission::Replay(value)",
  "Admission::ReplayError(error)",
  "Admission::Run(lease)",
  "idempotency::complete(&txn, lease, &response)",
  "idempotency::fail(&self.db, lease, &mapped)",
  "txn.commit().await",
  "pub(crate) async fn create_cart_with_channel_in_txn",
  "pub(crate) async fn add_line_item_with_pricing_adjustment_in_txn",
  "pub(crate) async fn update_context_in_txn",
  "pub(crate) async fn update_line_item_quantity_in_txn",
  "pub(crate) async fn update_line_item_pricing_in_txn",
  "pub(crate) async fn reprice_line_items_in_txn",
  "pub(crate) async fn remove_line_item_in_txn",
]) need(cartService, value, "Cart durable receipt owner boundary");

for (const value of [
  "self.run_storefront_idempotent_write(",
  '"storefront.create_cart"',
  '"storefront.add_line_item"',
  '"storefront.update_context"',
  '"storefront.update_line_item_quantity"',
  '"storefront.update_line_item_pricing"',
  '"storefront.remove_line_item"',
  '"storefront.reprice_line_items"',
]) need(cartPorts, value, "Cart storefront port receipt operation");

for (const value of [
  "Uuid::new_v4()",
  "with_idempotency_key(correlation_id)",
  "is_write: bool",
]) forbid(cartService, value, "synthetic Cart owner idempotency");

if (failures.length) {
  console.error("Commerce Store Cart idempotency verification failed:");
  for (const failure of failures) console.error("- " + failure);
  process.exit(Math.min(failures.length, 255));
}

console.log("Commerce Store Cart writes require caller-owned idempotency and reads remain observational");
