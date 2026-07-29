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
    failures.push(`${relativePath}: required HTTP checkout runtime file is missing`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function requireText(source, marker, description) {
  if (!source.includes(marker)) failures.push(`${description}: missing ${marker}`);
}

function findFunctionBody(source, functionName) {
  const signature = new RegExp(`(?:pub\\s+)?(?:async\\s+)?fn\\s+${functionName}\\s*\\(`, "g");
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

const runtimePath = "crates/rustok-commerce/src/controllers/mod.rs";
const checkoutPath = "crates/rustok-commerce/src/controllers/store/checkout.rs";
const stagedPath = "crates/rustok-commerce/src/storefront_staged_checkout_runtime.rs";
const registryPath = "crates/rustok-product/contracts/product-fba-registry.json";
const planPath = "crates/rustok-product/docs/implementation-plan.md";
const runtime = read(runtimePath);
const checkout = read(checkoutPath);
const staged = read(stagedPath);
const registrySource = read(registryPath);
const plan = read(planPath);

for (const marker of [
  "product_catalog_read_runtime: rustok_product::ProductCatalogReadRuntime",
  "shared_get::<rustok_product::ProductCatalogReadRuntime>()",
  "Commerce HTTP routes require ProductCatalogReadRuntime in HostRuntimeContext",
  "fn product_catalog_read_port",
  "self.product_catalog_read_runtime.read_port()",
]) {
  requireText(runtime, marker, "Commerce HTTP runtime");
}

const handlerBody = findFunctionBody(checkout, "complete_cart_checkout");
if (!handlerBody) {
  failures.push("Commerce HTTP checkout handler body is missing");
} else {
  requireText(
    handlerBody,
    "complete_storefront_checkout_input_with_product_port",
    "Commerce HTTP checkout handler",
  );
  requireText(
    handlerBody,
    "runtime.product_catalog_read_port()",
    "Commerce HTTP checkout handler",
  );
  if (handlerBody.includes("complete_storefront_checkout_input(")) {
    failures.push("Commerce HTTP checkout must not call the embedded compatibility wrapper");
  }
  if (handlerBody.includes("CatalogService::new")) {
    failures.push("Commerce HTTP checkout must not construct CatalogService");
  }
}

const composedBody = findFunctionBody(
  staged,
  "complete_storefront_checkout_input_with_product_port",
);
if (!composedBody) {
  failures.push("Commerce composed staged checkout body is missing");
} else if (composedBody.includes("CatalogService::new")) {
  failures.push("Commerce composed staged checkout must not construct CatalogService");
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
  if (!complete.includes("commerce-checkout-http")) {
    failures.push("Product FBA registry must mark commerce-checkout-http source-complete");
  }
  if (pending.includes("commerce-checkout-http")) {
    failures.push("Product FBA registry must remove commerce-checkout-http from pending consumers");
  }
  if (!pending.includes("commerce-checkout-graphql")) {
    failures.push("Product FBA registry must keep commerce-checkout-graphql pending");
  }
  if (composition.status !== "source_complete_consumer_cutover_partial") {
    failures.push("Product FBA registry must retain partial consumer-cutover status");
  }
}

for (const marker of [
  "Commerce HTTP checkout",
  "Commerce GraphQL checkout",
  "verify-product-http-checkout-catalog-runtime.mjs",
  "remains open only for that surface",
]) {
  requireText(plan, marker, "Product implementation plan");
}

if (failures.length > 0) {
  console.error("product HTTP checkout catalog runtime verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("product HTTP checkout catalog runtime verification passed");
