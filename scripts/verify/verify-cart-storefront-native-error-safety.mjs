#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? path.resolve(configuredRoot)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const read = (relativePath) => readFileSync(path.join(root, relativePath), "utf8");
const failures = [];
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const transport = read("crates/rustok-cart/storefront/src/transport/mod.rs");
const safe = read(
  "crates/rustok-cart/storefront/src/transport/native_server_adapter_ssr.rs",
);
const mapping = read(
  "crates/rustok-cart/storefront/src/transport/native_server_mapping.rs",
);
const evidence = JSON.parse(
  read(
    "crates/rustok-cart/contracts/evidence/storefront-native-error-safety-source.json",
  ),
);

for (const [value, label] of [
  ['#[cfg(not(feature = "ssr"))]\nmod native_server_adapter;', "client contract selection"],
  ['#[cfg(feature = "ssr")]\n#[path = "native_server_adapter_ssr.rs"]\nmod native_server_adapter;', "safe SSR selection"],
  ['#[cfg(feature = "ssr")]\nmod native_server_mapping;', "SSR mapping selection"],
]) requireText(transport, value, label);

for (const [value, label] of [
  ["const CART_STOREFRONT_NATIVE_BOUNDARY", "native boundary constant"],
  ["fn tenant_context_error", "tenant context mapper"],
  ["fn auth_context_error", "auth context mapper"],
  ["fn transactional_event_bus_from_runtime", "runtime dependency mapper"],
  ["fn cart_input_error", "input mapper"],
  ["fn customer_error", "customer mapper"],
  ["fn cart_error", "cart owner mapper"],
  ["fn pricing_error", "pricing port mapper"],
  ['owner_operation = "extract_tenant_context"', "tenant extraction operation"],
  ['owner_operation = "extract_optional_auth_context"', "auth extraction operation"],
  ['"Storefront tenant context is temporarily unavailable"', "tenant public envelope"],
  ['"Storefront authentication context is temporarily unavailable"', "auth public envelope"],
  ['"Cart runtime is temporarily unavailable"', "runtime public envelope"],
  ['"Invalid cart selection"', "cart input public envelope"],
  ['"Invalid cart line item selection"', "line-item input public envelope"],
  ['"Customer information is temporarily unavailable"', "customer public envelope"],
  ['"Cart is temporarily unavailable"', "cart storage public envelope"],
  ["error = ?error", "original cause diagnostics"],
  ["request_tenant_id = ?request_context.map", "request tenant diagnostics"],
  ["tenant_id = %tenant_id", "tenant diagnostics"],
  ["owner_code = %error.code", "pricing owner code diagnostics"],
  ["ServerFnError::new(error.message)", "sanitized pricing message forwarding"],
]) requireText(safe, value, label);

for (const value of [
  ".map_err(ServerFnError::new)",
  "ServerFnError::new(error.to_string())",
  "ServerFnError::new(err.to_string())",
  "ServerFnError::new(format!(",
  "Err(ServerFnError::new(err.to_string()))",
  'requires TransactionalEventBus in host runtime context',
  "request_context.correlation_id",
]) forbidText(safe, value, "safe cart SSR adapter");

for (const [value, label] of [
  ["pub(super) fn map_native_cart", "shared cart DTO mapper"],
  ["pub(super) fn storefront_cart_pricing_update", "shared pricing snapshot mapper"],
  ["StorefrontCartShippingOption", "shipping option DTO preservation"],
  ["CartLineItemPricingUpdate", "pricing update preservation"],
]) requireText(mapping, value, label);

if (evidence.status !== "storefront_native_error_safety_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  ssr_safe_adapter_selected: true,
  client_server_function_contract_preserved: true,
  tenant_context_static_public_envelope: true,
  auth_context_static_public_envelope: true,
  runtime_dependency_static_public_envelope: true,
  cart_input_static_public_envelopes: true,
  customer_raw_error_public: false,
  cart_raw_error_public: false,
  pricing_port_message_remains_domain_sanitized: true,
  owner_context_logging: true,
  cart_dto_changed: false,
  transport_selection_changed: false,
  graphql_transport_changed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence source_contract.${key} must be ${expected}`);
  }
}
for (const key of [
  "tests_run",
  "cargo_run",
  "format_run",
  "verifiers_run",
  "workflow_checks_run",
  "ci_run",
  "native_runtime_proven",
  "graphql_runtime_proven",
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`evidence validation.${key} must remain false`);
  }
}

if (failures.length > 0) {
  console.error("Cart storefront native error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ cart storefront SSR uses static public envelopes with private owner causes; source evidence remains unvalidated",
);
