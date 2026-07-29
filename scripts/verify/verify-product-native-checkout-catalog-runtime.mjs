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
  const signature = new RegExp(`pub\s+async\s+fn\s+${functionName}\s*\(`, "g");
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
const cargoPath = "crates/rustok-order/storefront/Cargo.toml";
const registryPath = "crates/rustok-product/contracts/product-fba-registry.json";
const planPath = "crates/rustok-product/docs/implementation-plan.md";
const staged = read(stagedPath);
const native = read(nativePath);
const cargo = read(cargoPath);
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
if (!compatibilityBody) {
  failures.push("Commerce checkout compatibility wrapper is missing");
} else {
  requireText(
    compatibilityBody,
    "product_catalog_read_runtime_for_current_graphql_scope",
    "Commerce checkout compatibility wrapper",
  );
  if (compatibilityBody.includes("CatalogService::new")) {
    failures.push("Commerce checkout compatibility wrapper must not construct CatalogService");
  }
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
for (const marker of [
  '"dep:rustok-product"',
  "rustok-product = { workspace = true, optional = true }",
]) {
  requireText(cargo, marker, "Order storefront SSR dependency");
}
const hydrateFeature = cargo.match(/hydrate\s*=\s*\[([^\]]*)\]/)?.[1] ?? "";
if (hydrateFeature.includes("rustok-product")) {
  failures.push("Order storefront hydrate feature must not include rustok-product backend dependency");
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
  for (const consumer of [
    "order-storefront-native",
    "commerce-checkout-http",
    "commerce-checkout-graphql",
  ]) {
    if (!complete.includes(consumer)) {
      failures.push(`Product FBA registry must mark ${consumer} source-complete`);
    }
  }
  if (pending.length !== 0) {
    failures.push("Product FBA registry must have no pending checkout consumers");
  }
  if (composition.status !== "source_complete_consumer_cutover_complete") {
    failures.push("Product FBA registry must record completed consumer cutover");
  }
}
for (const marker of [
  "Order storefront native checkout",
  "checkout consumer source cutover is complete",
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
