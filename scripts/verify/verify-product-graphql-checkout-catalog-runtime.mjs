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
    failures.push(`${relativePath}: required GraphQL checkout runtime file is missing`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function requireText(source, marker, description) {
  if (!source.includes(marker)) failures.push(`${description}: missing ${marker}`);
}

function findFunctionBody(source, functionName) {
  const signature = new RegExp(`(?:pub\\s+)?(?:crate\\s+)?(?:async\\s+)?fn\\s+${functionName}\\s*\\(`, "g");
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

const graphqlRuntimePath = "crates/rustok-commerce/src/graphql_runtime.rs";
const stagedPath = "crates/rustok-commerce/src/storefront_staged_checkout_runtime.rs";
const mutationPath = "crates/rustok-commerce/src/graphql/mutations/checkout.rs";
const registryPath = "crates/rustok-product/contracts/product-fba-registry.json";
const planPath = "crates/rustok-product/docs/implementation-plan.md";
const graphqlRuntime = read(graphqlRuntimePath);
const staged = read(stagedPath);
const mutation = read(mutationPath);
const registrySource = read(registryPath);
const plan = read(planPath);

for (const marker of [
  "use rustok_product::ProductCatalogReadRuntime;",
  "CURRENT_COMMERCE_PRODUCT_CATALOG_READ_RUNTIME",
  "runtime_data.product_catalog_read_runtime()",
  "product_catalog_read_runtime: ProductCatalogReadRuntime",
  "pub fn product_catalog_read_runtime(&self)",
  "shared_get::<ProductCatalogReadRuntime>()",
  "commerce GraphQL requires ProductCatalogReadRuntime in host composition",
  "product_catalog_read_runtime_for_current_graphql_scope",
  "ProductCatalogReadRuntime::in_process(db, event_bus)",
]) {
  requireText(graphqlRuntime, marker, "Commerce GraphQL runtime");
}

const scopeHelper = findFunctionBody(
  graphqlRuntime,
  "product_catalog_read_runtime_for_current_graphql_scope",
);
if (!scopeHelper) {
  failures.push("Commerce GraphQL Product runtime scope helper is missing");
} else {
  requireText(scopeHelper, "try_with(Clone::clone)", "Commerce GraphQL Product runtime scope helper");
  requireText(scopeHelper, "ProductCatalogReadRuntime::in_process", "Commerce GraphQL Product runtime scope helper");
}

const wrapperBody = findFunctionBody(staged, "complete_storefront_checkout_input");
if (!wrapperBody) {
  failures.push("Commerce GraphQL checkout compatibility wrapper is missing");
} else {
  requireText(
    wrapperBody,
    "product_catalog_read_runtime_for_current_graphql_scope",
    "Commerce GraphQL checkout compatibility wrapper",
  );
  requireText(wrapperBody, ".read_port()", "Commerce GraphQL checkout compatibility wrapper");
  requireText(
    wrapperBody,
    "complete_storefront_checkout_input_with_product_port",
    "Commerce GraphQL checkout compatibility wrapper",
  );
  if (wrapperBody.includes("CatalogService::new")) {
    failures.push("Commerce GraphQL checkout compatibility wrapper must not construct CatalogService");
  }
}

const mutationBody = findFunctionBody(mutation, "complete_storefront_checkout");
if (!mutationBody) {
  failures.push("Commerce GraphQL checkout mutation body is missing");
} else {
  requireText(
    mutationBody,
    "complete_storefront_checkout_input",
    "Commerce GraphQL checkout mutation",
  );
  if (mutationBody.includes("CatalogService::new")) {
    failures.push("Commerce GraphQL checkout mutation must not construct CatalogService");
  }
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
  if (!complete.includes("commerce-checkout-graphql")) {
    failures.push("Product FBA registry must mark commerce-checkout-graphql source-complete");
  }
  if (pending.length !== 0) {
    failures.push("Product FBA registry must have no pending consumers");
  }
  if (composition.status !== "source_complete_consumer_cutover_complete") {
    failures.push("Product FBA registry must record completed consumer cutover");
  }
  if (
    registry.evidence?.graphql_checkout_runtime_verifier !==
    "scripts/verify/verify-product-graphql-checkout-catalog-runtime.mjs"
  ) {
    failures.push("Product FBA registry must link the GraphQL checkout verifier");
  }
}

for (const marker of [
  "mounted Commerce GraphQL checkout",
  "resolver-scoped task-local",
  "checkout consumer source cutover is complete",
  "verify-product-graphql-checkout-catalog-runtime.mjs",
  "Concrete external transport execution remains open",
]) {
  requireText(plan, marker, "Product implementation plan");
}

if (failures.length > 0) {
  console.error("product GraphQL checkout catalog runtime verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("product GraphQL checkout catalog runtime verification passed");
