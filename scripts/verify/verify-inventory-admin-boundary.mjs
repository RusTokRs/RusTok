#!/usr/bin/env node
// RusTok inventory admin boundary guardrails.
// Fast source-level checks for Wave 5 inventory-owned transport/write semantics.

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function readRepo(relativePath) {
  return readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function fail(message) {
  failures.push(message);
}

function assertContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) {
    fail(description);
  }
}

function assertNotContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (found) {
    fail(description);
  }
}

function functionBody(text, functionName) {
  const signature = new RegExp(`(?:pub(?:\\([^)]*\\))?\\s+)?(?:async\\s+)?fn\\s+${functionName}\\s*\\(`);
  const match = signature.exec(text);
  if (!match) {
    fail(`missing function ${functionName}`);
    return "";
  }

  const openBrace = text.indexOf("{", match.index);
  if (openBrace === -1) {
    fail(`missing body for function ${functionName}`);
    return "";
  }

  let depth = 0;
  for (let index = openBrace; index < text.length; index += 1) {
    const char = text[index];
    if (char === "{") {
      depth += 1;
    } else if (char === "}") {
      depth -= 1;
      if (depth === 0) {
        return text.slice(openBrace, index + 1);
      }
    }
  }

  fail(`unterminated body for function ${functionName}`);
  return "";
}

function assertInventoryServiceWriteResults() {
  const relativePath = "crates/rustok-inventory/src/services/inventory.rs";
  const source = readRepo(relativePath);
  const adjustFacade = functionBody(source, "adjust_variant_quantity");
  const setFacade = functionBody(source, "set_variant_quantity");
  const adjustUpdate = functionBody(source, "adjust_inventory_update");
  const setUpdate = functionBody(source, "set_inventory_update");

  assertContains(
    source,
    /fn\s+from_quantity_and_policy\s*\(quantity:\s*i32,\s*inventory_policy:\s*&str\)[\s\S]*quantity\s*>\s*0\s*\|\|\s*inventory_policy_allows_backorder\(inventory_policy\)/,
    `${relativePath}: InventoryQuantityWriteResult must derive in_stock from quantity plus backorder policy`,
  );
  assertContains(
    source,
    /struct\s+InventoryQuantityUpdate\s*\{[\s\S]*quantity:\s*i32,[\s\S]*inventory_policy:\s*String,[\s\S]*\}/,
    `${relativePath}: internal InventoryQuantityUpdate must carry quantity and inventory_policy`,
  );

  for (const [name, body, helperName] of [
    ["adjust_variant_quantity", adjustFacade, "adjust_inventory_update"],
    ["set_variant_quantity", setFacade, "set_inventory_update"],
  ]) {
    assertContains(
      body,
      helperName,
      `${relativePath}: ${name} must use ${helperName} instead of re-deriving policy state`,
    );
    assertContains(
      body,
      /InventoryQuantityWriteResult::from_quantity_and_policy\(\s*update\.quantity,\s*&update\.inventory_policy,\s*\)/,
      `${relativePath}: ${name} must build policy-aware typed write result from committed update`,
    );
    assertNotContains(
      body,
      "load_variant(&self.db",
      `${relativePath}: ${name} must not pre-read variant policy outside the mutation path`,
    );
  }

  for (const [name, body] of [
    ["adjust_inventory_update", adjustUpdate],
    ["set_inventory_update", setUpdate],
  ]) {
    assertContains(
      body,
      "let inventory_policy = variant.inventory_policy.clone();",
      `${relativePath}: ${name} must preserve the policy loaded inside the mutation path`,
    );
    assertContains(
      body,
      "Ok(InventoryQuantityUpdate",
      `${relativePath}: ${name} must return InventoryQuantityUpdate`,
    );
    assertContains(
      body,
      "inventory_policy,",
      `${relativePath}: ${name} must return inventory_policy with the committed quantity`,
    );
  }

  assertContains(
    source,
    "quantity_write_result_honors_backorder_policy_for_native_write_facades",
    `${relativePath}: missing targeted policy-aware write result regression test`,
  );
}

function assertInventoryAdminTransportBoundary() {
  const transportPath = "crates/rustok-inventory/admin/src/transport.rs";
  const transport = readRepo(transportPath);
  const apiPath = "crates/rustok-inventory/admin/src/api.rs";
  const api = readRepo(apiPath);
  const nativePath = "crates/rustok-inventory/admin/src/native.rs";
  const native = readRepo(nativePath);

  for (const marker of ["const BOOTSTRAP_QUERY", "const PRODUCTS_QUERY", "const PRODUCT_QUERY"]) {
    assertContains(transport, marker, `${transportPath}: transitional adapter must remain read-capable via ${marker}`);
  }
  for (const forbidden of [
    "mutation ",
    "setVariantQuantity",
    "adjustVariantQuantity",
    "reserveVariantQuantity",
    "releaseReservation",
    "checkVariantAvailability",
  ]) {
    assertNotContains(transport, forbidden, `${transportPath}: transitional GraphQL adapter must remain read-only and not contain ${forbidden}`);
  }

  for (const [functionName, nativeCall] of [
    ["set_variant_quantity", "crate::native::set_variant_quantity"],
    ["adjust_variant_quantity", "crate::native::adjust_variant_quantity"],
    ["reserve_variant_quantity", "crate::native::reserve_variant_quantity"],
    ["release_reservation_quantity", "crate::native::release_reservation_quantity"],
    ["check_variant_availability", "crate::native::check_variant_availability"],
  ]) {
    const body = functionBody(api, functionName);
    assertContains(body, nativeCall, `${apiPath}: ${functionName} must use the inventory-owned native facade`);
    assertNotContains(body, "fallback_", `${apiPath}: ${functionName} must not use transitional GraphQL fallback`);
    assertNotContains(body, "transitional_read_transport", `${apiPath}: ${functionName} must not use transitional read transport`);
  }

  for (const endpoint of [
    'endpoint = "inventory/variant/set-quantity"',
    'endpoint = "inventory/variant/adjust-quantity"',
    'endpoint = "inventory/variant/reserve-quantity"',
    'endpoint = "inventory/variant/release-reservation"',
    'endpoint = "inventory/variant/check-availability"',
  ]) {
    assertContains(native, endpoint, `${nativePath}: missing native server-function endpoint ${endpoint}`);
  }
}

assertInventoryServiceWriteResults();
assertInventoryAdminTransportBoundary();

if (failures.length > 0) {
  console.error("Inventory admin boundary check failed:");
  failures.forEach((failure) => console.error(`✗ ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

console.log("✔ Inventory admin boundary invariants passed");
