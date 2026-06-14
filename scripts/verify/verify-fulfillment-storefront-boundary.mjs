#!/usr/bin/env node
// RusTok fulfillment storefront FFA boundary guardrails.
// Fast source-level checks for module-owned shipping selection UI/core ownership.

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function readRepo(relativePath) {
  return readFileSync(repoPath(relativePath), "utf8");
}

function fail(message) {
  failures.push(message);
}

function assertExists(relativePath, description) {
  if (!existsSync(repoPath(relativePath))) fail(description);
}

function assertContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) fail(description);
}

function assertNotContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (found) fail(description);
}

const libPath = "crates/rustok-fulfillment/storefront/src/lib.rs";
const modelPath = "crates/rustok-fulfillment/storefront/src/model.rs";
const corePath = "crates/rustok-fulfillment/storefront/src/core/mod.rs";
const uiPath = "crates/rustok-fulfillment/storefront/src/ui/leptos.rs";
const commerceUiPath = "crates/rustok-commerce/storefront/src/ui/leptos/mod.rs";
const planPath = "crates/rustok-fulfillment/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";
const packagePath = "package.json";

for (const filePath of [libPath, modelPath, corePath, uiPath, commerceUiPath, planPath, registryPath, packagePath]) {
  assertExists(filePath, `${filePath}: expected fulfillment storefront FFA file`);
}

const lib = readRepo(libPath);
const model = readRepo(modelPath);
const core = readRepo(corePath);
const ui = readRepo(uiPath);
const commerceUi = readRepo(commerceUiPath);
const plan = readRepo(planPath);
const registry = readRepo(registryPath);
const packageJson = readRepo(packagePath);

for (const marker of [
  "mod model;",
  "pub mod core;",
  "FulfillmentShippingSelectionPanel",
  "StorefrontDeliveryGroup",
  "StorefrontShippingOption",
]) {
  assertContains(lib, marker, `${libPath}: expected storefront public surface marker ${marker}`);
}

for (const marker of ["StorefrontDeliveryGroup", "StorefrontShippingOption", "Serialize", "Deserialize"]) {
  assertContains(model, marker, `${modelPath}: expected module-owned DTO marker ${marker}`);
}

for (const marker of [
  "SelectShippingOptionRequest",
  "ShippingSelectionLabels",
  "build_select_shipping_option_request",
  "format_shipping_option_price",
]) {
  assertContains(core, marker, `${corePath}: expected core-owned selection helper ${marker}`);
}
for (const marker of ["leptos::", "#[component]", "#[server", "GraphqlRequest", "web_sys::"]) {
  assertNotContains(core, marker, `${corePath}: core must stay UI/transport free (${marker})`);
}

for (const marker of [
  "FulfillmentShippingSelectionPanel",
  "build_select_shipping_option_request",
  "on_select_shipping_option",
  "StorefrontDeliveryGroup",
  "StorefrontShippingOption",
]) {
  assertContains(ui, marker, `${uiPath}: expected fulfillment-owned selection UI marker ${marker}`);
}
for (const marker of ["crate::api", "rustok_commerce::", "GraphqlRequest", "#[server"]) {
  assertNotContains(ui, marker, `${uiPath}: UI adapter must not call commerce transport directly (${marker})`);
}

assertContains(commerceUi, "FulfillmentShippingSelectionPanel", `${commerceUiPath}: commerce host must render fulfillment-owned selection UI`);
assertContains(commerceUi, "transport::select_storefront_shipping_option", `${commerceUiPath}: commerce host may keep transitional aggregate transport callback`);
assertContains(plan, "verify-fulfillment-storefront-boundary.mjs", `${planPath}: local plan must mention storefront boundary guardrail`);
assertContains(registry, "verify-fulfillment-storefront-boundary.mjs", `${registryPath}: central registry must mention storefront boundary guardrail`);
assertContains(packageJson, "verify:fulfillment:storefront-boundary", `${packagePath}: expected fulfillment storefront boundary script`);
assertContains(packageJson, "npm run verify:fulfillment:storefront-boundary", `${packagePath}: aggregate FFA migration verification must include storefront fulfillment boundary`);

if (failures.length > 0) {
  console.error("fulfillment storefront boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("fulfillment storefront boundary verification passed");
