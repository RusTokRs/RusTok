#!/usr/bin/env node
// RusTok payment storefront FFA boundary guardrails.
// Fast source-level checks for payment-owned checkout action/card UI and request ownership.

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

const libPath = "crates/rustok-payment/storefront/src/lib.rs";
const corePath = "crates/rustok-payment/storefront/src/core.rs";
const transportPath = "crates/rustok-payment/storefront/src/transport.rs";
const uiPath = "crates/rustok-payment/storefront/src/ui/leptos.rs";
const commerceUiPath = "crates/rustok-commerce/storefront/src/ui/leptos/mod.rs";
const commerceRequestsPath = "crates/rustok-commerce/storefront/src/core/requests.rs";
const planPath = "crates/rustok-payment/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";

for (const filePath of [libPath, corePath, transportPath, uiPath, commerceUiPath, commerceRequestsPath, planPath, registryPath]) {
  assertExists(filePath, `${filePath}: expected payment storefront FFA file`);
}

const lib = readRepo(libPath);
const core = readRepo(corePath);
const transport = readRepo(transportPath);
const ui = readRepo(uiPath);
const commerceUi = readRepo(commerceUiPath);
const commerceRequests = readRepo(commerceRequestsPath);
const plan = readRepo(planPath);
const registry = readRepo(registryPath);

for (const marker of ["pub mod core;", "pub mod transport;", "PaymentCollectionActionButton", "PaymentCollectionCard"]) {
  assertContains(lib, marker, `${libPath}: expected storefront public surface marker ${marker}`);
}

for (const marker of [
  "PaymentCollectionCardData",
  "PaymentCollectionCardViewModel",
  "build_payment_collection_card_view_model",
  "PaymentCollectionActionLabels",
  "payment_collection_action_label",
]) {
  assertContains(core, marker, `${corePath}: expected core-owned payment presentation marker ${marker}`);
}
for (const marker of ["leptos::", "#[component]", "#[server", "GraphqlRequest", "web_sys::"]) {
  assertNotContains(core, marker, `${corePath}: core must stay UI/transport free (${marker})`);
}

for (const marker of [
  "PaymentCollectionCreateRequest",
  "build_payment_collection_create_request",
  "normalize_required",
]) {
  assertContains(transport, marker, `${transportPath}: expected transport-owned request marker ${marker}`);
}
for (const marker of ["leptos::", "#[component]", "#[server", "GraphqlRequest", "web_sys::"]) {
  assertNotContains(transport, marker, `${transportPath}: transport facade must stay framework/native-endpoint free (${marker})`);
}

for (const marker of [
  "PaymentCollectionActionButton",
  "PaymentCollectionCard",
  "PaymentCollectionCreateRequest",
  "build_payment_collection_create_request",
  "on_create_payment_collection: Callback<PaymentCollectionCreateRequest>",
]) {
  assertContains(ui, marker, `${uiPath}: expected payment-owned UI/request marker ${marker}`);
}
for (const marker of ["crate::api", "rustok_commerce::", "GraphqlRequest", "#[server"] ) {
  assertNotContains(ui, marker, `${uiPath}: UI adapter must not call commerce/raw transport directly (${marker})`);
}

assertContains(commerceUi, "PaymentCollectionActionButton", `${commerceUiPath}: commerce host must render payment-owned action UI`);
assertContains(commerceUi, "Callback::new(move |request: PaymentCollectionCreateRequest|", `${commerceUiPath}: commerce callback must accept payment-owned request DTO`);
assertNotContains(commerceUi, "build_payment_collection_command_request", `${commerceUiPath}: commerce UI must not rebuild payment requests from raw cart ids`);
assertContains(commerceRequests, "pub type PaymentCollectionCommandRequest = PaymentCollectionCreateRequest", `${commerceRequestsPath}: commerce transport may keep transitional alias to owner request`);
assertNotContains(commerceRequests, "build_payment_collection_create_request", `${commerceRequestsPath}: commerce core must not wrap payment-owned request construction`);
assertNotContains(commerceRequests, "build_payment_collection_command_request", `${commerceRequestsPath}: commerce core must not expose a payment request builder after owner UI handoff`);
assertContains(plan, "verify-payment-storefront-boundary.mjs", `${planPath}: local plan must mention payment storefront boundary guardrail`);
assertContains(registry, "verify-payment-storefront-boundary.mjs", `${registryPath}: central registry must mention payment storefront boundary guardrail`);

if (failures.length > 0) {
  console.error("payment storefront boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("payment storefront boundary verification passed");
