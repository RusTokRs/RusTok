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
const countText = (source, value) => source.split(value).length - 1;
const functionBody = (source, functionName) => {
  const signature = new RegExp(`fn\\s+${functionName}\\s*(?:<[^>]*>)?\\s*\\(`);
  const match = signature.exec(source);
  if (!match) {
    failures.push(`missing function body for ${functionName}`);
    return "";
  }
  const openBrace = source.indexOf("{", match.index);
  if (openBrace === -1) {
    failures.push(`missing opening brace for ${functionName}`);
    return "";
  }
  let depth = 0;
  for (let index = openBrace; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(openBrace, index + 1);
    }
  }
  failures.push(`unterminated function body for ${functionName}`);
  return "";
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
const doc = read("crates/rustok-cart/docs/storefront-native-error-safety.md");

for (const [value, label] of [
  ['#[cfg(not(feature = "ssr"))]\nmod native_server_adapter;', "client contract selection"],
  ['#[cfg(feature = "ssr")]\n#[path = "native_server_adapter_ssr.rs"]\nmod native_server_adapter;', "safe SSR selection"],
  ['#[cfg(feature = "ssr")]\nmod native_server_mapping;', "SSR mapping selection"],
]) requireText(transport, value, label);

for (const [value, label] of [
  ["const CART_STOREFRONT_NATIVE_BOUNDARY", "native boundary constant"],
  ["fn context_extraction_error<E>(", "shared context extraction mapper"],
  ["fn tenant_context_error<E>(", "tenant context mapper"],
  ["fn auth_context_error<E>(", "auth context mapper"],
  ["fn transactional_event_bus_from_runtime", "runtime dependency mapper"],
  ["fn cart_input_error", "input mapper"],
  ["fn customer_error", "customer mapper"],
  ["fn cart_error", "cart owner mapper"],
  ["fn pricing_error", "pricing port mapper"],
  ['"extract_tenant_context"', "tenant extraction operation"],
  ['"extract_optional_auth_context"', "auth extraction operation"],
  ['"Storefront tenant context is temporarily unavailable"', "tenant public envelope"],
  ['"Storefront authentication context is temporarily unavailable"', "auth public envelope"],
  ['"Cart runtime is temporarily unavailable"', "runtime public envelope"],
  ['"Invalid cart selection"', "cart input public envelope"],
  ['"Invalid cart line item selection"', "line-item input public envelope"],
  ['"Customer information is temporarily unavailable"', "customer public envelope"],
  ['"Cart is temporarily unavailable"', "cart storage public envelope"],
  ["owner_code = %error.code", "pricing owner code diagnostics"],
  ["ServerFnError::new(error.message)", "sanitized pricing message forwarding"],
  [
    "Err(rustok_customer::CustomerError::CustomerByUserNotFound(_)) => Ok(None)",
    "customer not-found behavior",
  ],
]) requireText(safe, value, label);

for (const obsolete of [
  "fn context_extraction_error<E: std::fmt::Debug>(",
  "fn tenant_context_error<E: std::fmt::Debug>(",
  "fn auth_context_error<E: std::fmt::Debug>(",
]) forbidText(safe, obsolete, "obsolete cart storefront framework diagnostic contract");

const contextBody = functionBody(safe, "context_extraction_error");
requireText(
  contextBody,
  "let error_type = std::any::type_name::<E>();",
  "context extraction error type",
);
requireText(contextBody, "error_type", "context extraction type diagnostic");
for (const payload of [
  "error = ?error",
  "error = %error",
  "error = ?_error",
  "error = %_error",
]) forbidText(contextBody, payload, "complete framework context error payload");
if (countText(safe, "let error_type = std::any::type_name::<E>();") !== 1) {
  failures.push("expected exactly one shared framework context type-only diagnostic site");
}

const customerBody = functionBody(safe, "customer_error");
for (const [value, label] of [
  ["let error_type = std::any::type_name_of_val(&error);", "customer error type"],
  ["error_type", "customer type-only diagnostic"],
  [
    "correlation_id = ?request_context.map(|context| context.correlation_id)",
    "customer correlation diagnostic",
  ],
  ["request_context_present = request_context.is_some()", "request context presence"],
  [
    "request_tenant_id_non_nil = ?request_context.map(|context| !context.tenant_id.is_nil())",
    "request tenant shape",
  ],
  ["tenant_id_non_nil = !tenant_id.is_nil()", "tenant shape"],
  ["user_id_non_nil = !user_id.is_nil()", "user shape"],
  ["channel_id_present", "channel id presence"],
  ["channel_id_non_nil", "channel id shape"],
  ["channel_slug_present", "channel slug presence"],
  ["channel_slug_length", "channel slug length"],
  ["locale_present", "locale presence"],
  ["locale_length", "locale length"],
  ['code = "cart.storefront_customer_unavailable"', "customer diagnostic code"],
  [
    'ServerFnError::new("Customer information is temporarily unavailable")',
    "customer static public envelope",
  ],
]) requireText(customerBody, value, label);
for (const payload of [
  "error = ?error",
  "error = %error",
  "request_tenant_id = ?request_context.map",
  "tenant_id = %tenant_id",
  "user_id = %user_id",
  "channel_id = ?request_context",
  "channel_slug = ?request_context",
  "locale = ?request_context",
]) forbidText(customerBody, payload, "complete customer cause or raw identity/context diagnostic");
if (countText(safe, "let error_type = std::any::type_name_of_val(&error);") !== 1) {
  failures.push("expected exactly one customer type-only diagnostic site");
}
if (
  countText(
    safe,
    "correlation_id = ?request_context.map(|context| context.correlation_id)",
  ) !== 1
) {
  failures.push("expected exactly one optional customer correlation diagnostic site");
}
if (countText(safe, "error = ?error") !== 5) {
  failures.push(
    "cart input, Cart owner, and pricing owner diagnostics must remain unchanged in this bounded customer-only slice",
  );
}

for (const value of [
  ".map_err(ServerFnError::new)",
  "ServerFnError::new(error.to_string())",
  "ServerFnError::new(err.to_string())",
  "ServerFnError::new(format!(",
  "Err(ServerFnError::new(err.to_string()))",
  'requires TransactionalEventBus in host runtime context',
]) forbidText(safe, value, "safe cart SSR adapter");

for (const [value, label] of [
  ['endpoint = "cart/storefront-data"', "cart read endpoint"],
  ['endpoint = "cart/decrement-line-item"', "cart decrement endpoint"],
  ['endpoint = "cart/remove-line-item"', "cart remove endpoint"],
  ["pub(super) fn map_native_cart", "shared cart DTO mapper"],
  ["pub(super) fn storefront_cart_pricing_update", "shared pricing snapshot mapper"],
  ["StorefrontCartShippingOption", "shipping option DTO preservation"],
  ["CartLineItemPricingUpdate", "pricing update preservation"],
]) {
  const target = value.startsWith("endpoint") ? safe : mapping;
  requireText(target, value, label);
}

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
  framework_context_debug_bounds_removed: true,
  framework_context_error_type_only: true,
  complete_framework_context_error_logged: false,
  customer_static_public_envelope: true,
  customer_error_type_only: true,
  complete_customer_error_logged: false,
  customer_correlation_diagnostic: true,
  customer_identity_shape_only: true,
  customer_context_shape_only: true,
  raw_customer_identity_context_logged: false,
  customer_not_found_behavior_preserved: true,
  customer_raw_error_public: false,
  cart_raw_error_public: false,
  pricing_port_message_remains_domain_sanitized: true,
  owner_context_logging: true,
  cart_pricing_identifier_mapper_cleanup_out_of_scope: true,
  cart_dto_changed: false,
  transport_selection_changed: false,
  graphql_transport_changed: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`evidence source_contract.${key} must be ${expected}`);
  }
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
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

requireText(doc, "Status: `source_ready_unvalidated`", "documentation status");
requireText(
  doc,
  "Framework context extraction errors are recorded by Rust type only",
  "documentation framework diagnostic policy",
);
requireText(
  doc,
  "Customer lookup failures are recorded by concrete error type",
  "documentation customer diagnostic policy",
);
requireText(
  doc,
  "Cart owner, pricing, and identifier diagnostics remain separate open cleanup slices",
  "documentation remaining mapper boundary",
);

if (failures.length > 0) {
  console.error("Cart storefront native error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ cart storefront framework and customer diagnostics use bounded causes/context while Cart owner and pricing cleanup remains open; source evidence remains unvalidated",
);
