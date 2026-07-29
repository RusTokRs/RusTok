#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!existsSync(absolutePath)) {
    failures.push(`${relativePath}: required native checkout runtime file is missing`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function requireText(source, marker, description) {
  if (!source.includes(marker)) failures.push(`${description}: missing ${marker}`);
}

function findFunctionBody(source, functionName) {
  const signature = new RegExp(`pub\\s+async\\s+fn\\s+${functionName}\\s*\\(`, "g");
  const match = signature.exec(source);
  if (!match) return null;
  const openBrace = source.indexOf("{", match.index);
  if (openBrace === -1) return null;
  let depth = 0;
  for (let index = openBrace; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(openBrace + 1, index);
    }
  }
  return null;
}

const stagedPath = "crates/rustok-commerce/src/storefront_staged_checkout_runtime.rs";
const nativePath = "crates/rustok-order/storefront/src/transport/native_server_adapter/server_functions.rs";
const registryPath = "crates/rustok-product/contracts/product-fba-registry.json";
const planPath = "crates/rustok-product/docs/implementation-plan.md";
const staged = read(stagedPath);
const native = read(nativePath);
const registrySource = read(registryPath);
const plan = read(planPath);

for (const marker of [
  "complete_storefront_checkout_with_product_port",
  "complete_storefront_checkout_input_with_product_port",
  "product_catalog_read_port: Arc<dyn rustok_product::ProductCatalogReadPort>",
]) {
  requireText(staged, marker, "Commerce staged checkout");
}
const composedBody = findFunctionBody(
  staged,
  "complete_storefront_checkout_input_with_product_port",
);
if (!composedBody) {
  failures.push("Commerce staged checkout: composed input function body is missing");
} else {
  requireText(composedBody, "product_catalog_read_port", "Commerce composed checkout body");
  requireText(composedBody, "CheckoutPlanBuilder::new", "Commerce composed checkout body");
  if (composedBody.includes("CatalogService::new")) {
    failures.push("Commerce composed checkout body must not construct CatalogService");
  }
}
const compatibilityBody = findFunctionBody(staged, "complete_storefront_checkout_input");
if (!compatibilityBody?.includes("CatalogService::new")) {
  failures.push("Commerce compatibility wrapper must remain explicit until HTTP/GraphQL cutover");
}
for (const marker of [
  "shared_get::<ProductCatalogReadRuntime>()",
  ".read_port()",
  "complete_storefront_checkout_with_product_port",
  'dependency = "ProductCatalogReadRuntime"',
]) {
  requireText(native, marker, "Order native checkout");
}
if (native.includes("CatalogService::new")) {
  failures.push("Order native checkout must not construct CatalogService");
}

let registry;
try {
  registry = JSON.parse(registrySource);
} catch (error) {
  failures.push(`Product FBA registry is invalid JSON: ${error.message}`);
}
if (registry) {
  const composition = registry.runtime_composition ?? {};
  const complete = composition.source_complete_consumers ?? [];
  const pending = composition.pending_consumers ?? [];
  if (!complete.includes("order-storefront-native")) {
    failures.push("Product FBA registry must mark order-storefront-native source-complete");
  }
  if (pending.includes("order-storefront-native")) {
    failures.push("Product FBA registry must remove order-storefront-native from pending consumers");
  }
  for (const consumer of ["commerce-checkout-http", "commerce-checkout-graphql"]) {
    if (!pending.includes(consumer)) {
      failures.push(`Product FBA registry must keep ${consumer} pending`);
    }
  }
}
for (const marker of [
  "Order storefront native checkout",
  "complete_storefront_checkout_with_product_port",
  "Commerce HTTP and GraphQL checkout",
  "verify-product-native-checkout-catalog-runtime.mjs",
]) {
  requireText(plan, marker, "Product implementation plan");
}

if (failures.length > 0) {
  console.error("product native checkout catalog runtime verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("product native checkout catalog runtime verification passed");
