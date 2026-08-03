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
  ["fn missing_variant_error", "missing variant mapper"],
  ['"extract_tenant_context"', "tenant extraction operation"],
  ['"extract_optional_auth_context"', "auth extraction operation"],
  ['"Storefront tenant context is temporarily unavailable"', "tenant public envelope"],
  ['"Storefront authentication context is temporarily unavailable"', "auth public envelope"],
  ['"Cart runtime is temporarily unavailable"', "runtime public envelope"],
  ['"Invalid cart selection"', "cart input public envelope"],
  ['"Invalid cart line item selection"', "line-item input public envelope"],
  ['"Customer information is temporarily unavailable"', "customer public envelope"],
  ['"Cart is temporarily unavailable"', "cart storage public envelope"],
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
  [
    "correlation_id = ?request_context.map(|context| context.correlation_id)",
    "customer correlation diagnostic",
  ],
  ["request_context_present = request_context.is_some()", "customer request context presence"],
  ["request_tenant_id_non_nil", "customer request tenant shape"],
  ["tenant_id_non_nil = !tenant_id.is_nil()", "customer tenant shape"],
  ["user_id_non_nil = !user_id.is_nil()", "customer user shape"],
  ["channel_id_present", "customer channel presence"],
  ["channel_id_non_nil", "customer channel shape"],
  ["channel_slug_present", "customer slug presence"],
  ["channel_slug_length", "customer slug length"],
  ["locale_present", "customer locale presence"],
  ["locale_length", "customer locale length"],
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
if (countText(customerBody, "let error_type = std::any::type_name_of_val(&error);") !== 1) {
  failures.push("expected exactly one customer type-only diagnostic site");
}

const cartBody = functionBody(safe, "cart_error");
for (const [value, label] of [
  ["let error_type = std::any::type_name_of_val(&error);", "Cart owner error type"],
  ["let correlation_id = request_context.map", "Cart owner correlation derivation"],
  ["let request_context_present = request_context.is_some();", "Cart request context presence"],
  ["let request_tenant_id_non_nil", "Cart request tenant shape"],
  ["let tenant_id_non_nil = !tenant_id.is_nil();", "Cart tenant shape"],
  ["let cart_id_present = cart_id.is_some();", "Cart id presence"],
  ["let cart_id_non_nil = cart_id.map", "Cart id shape"],
  ["let line_item_id_present = line_item_id.is_some();", "Cart line-item presence"],
  ["let line_item_id_non_nil = line_item_id.map", "Cart line-item shape"],
  ["let channel_id_present", "Cart channel id presence"],
  ["let channel_id_non_nil", "Cart channel id shape"],
  ["let channel_slug_present", "Cart channel slug presence"],
  ["let channel_slug_length", "Cart channel slug length"],
  ["let locale_present", "Cart locale presence"],
  ["let locale_length", "Cart locale length"],
  ["correlation_id = ?correlation_id", "Cart owner correlation diagnostic"],
  ["public_retryable = retryable", "Cart retryability diagnostic"],
  ["ServerFnError::new(public_message)", "Cart public envelope mapping"],
  ["cart storefront owner operation failed", "Cart technical diagnostic message"],
  ["cart storefront owner operation was rejected", "Cart rejection diagnostic message"],
]) requireText(cartBody, value, label);
for (const payload of [
  "error = ?error",
  "error = %error",
  "request_tenant_id = ?request_context.map",
  "tenant_id = %tenant_id",
  "cart_id = ?cart_id",
  "cart_id = %cart_id",
  "line_item_id = ?line_item_id",
  "line_item_id = %line_item_id",
  "channel_id = ?request_context",
  "channel_slug = ?request_context",
  "locale = ?request_context",
]) forbidText(cartBody, payload, "complete Cart cause or raw identity/context diagnostic");
for (const [value, label] of [
  ["CartError::Validation(_)", "Cart validation mapping"],
  ["CartError::CartNotFound(_)", "Cart not-found mapping"],
  ["CartError::CartLineItemNotFound(_)", "Cart line-item not-found mapping"],
  ["CartError::InvalidTransition { .. }", "Cart transition mapping"],
  ["CartError::Database(_)", "Cart database mapping"],
  ["CartError::TaxBoundary {", "Cart tax-boundary mapping"],
  ['"cart.storefront_request_invalid"', "Cart validation code"],
  ['"cart.storefront_cart_not_found"', "Cart not-found code"],
  ['"cart.storefront_line_item_not_found"', "Cart line-item code"],
  ['"cart.storefront_state_conflict"', "Cart conflict code"],
  ['"cart.storefront_storage_unavailable"', "Cart storage code"],
  ['"cart.storefront_tax_invalid"', "Cart tax validation code"],
  ['"cart.storefront_tax_not_found"', "Cart tax not-found code"],
  ['"cart.storefront_tax_conflict"', "Cart tax conflict code"],
  ['"cart.storefront_tax_forbidden"', "Cart tax forbidden code"],
  ['"cart.storefront_tax_unavailable"', "Cart tax unavailable code"],
  ['"cart.storefront_tax_failed"', "Cart tax failure code"],
]) requireText(cartBody, value, label);
if (countText(cartBody, "tracing::error!(") !== 1) {
  failures.push("expected exactly one Cart owner technical diagnostic path");
}
if (countText(cartBody, "tracing::warn!(") !== 1) {
  failures.push("expected exactly one Cart owner rejection diagnostic path");
}
if (countText(cartBody, "correlation_id = ?correlation_id") !== 2) {
  failures.push("both Cart owner severity paths must retain optional correlation diagnostics");
}
if (countText(cartBody, "public_retryable = retryable") !== 2) {
  failures.push("both Cart owner severity paths must retain retryability diagnostics");
}

const pricingBody = functionBody(safe, "pricing_error");
for (const [value, label] of [
  ["let technical = matches!(", "pricing technical classification"],
  ["rustok_api::PortErrorKind::Unavailable", "pricing unavailable classification"],
  ["rustok_api::PortErrorKind::Timeout", "pricing timeout classification"],
  ["rustok_api::PortErrorKind::InvariantViolation", "pricing invariant classification"],
  ["let error_type = std::any::type_name_of_val(&error);", "pricing error type"],
  ["let correlation_id = request_context.map", "pricing correlation derivation"],
  ["let request_context_present = request_context.is_some();", "pricing request context presence"],
  ["let request_tenant_id_non_nil", "pricing request tenant shape"],
  ["let tenant_id_non_nil = !tenant_id.is_nil();", "pricing tenant shape"],
  ["let cart_id_non_nil = !cart_id.is_nil();", "pricing cart shape"],
  ["let line_item_id_non_nil = !line_item_id.is_nil();", "pricing line-item shape"],
  ["let channel_id_present", "pricing channel id presence"],
  ["let channel_id_non_nil", "pricing channel id shape"],
  ["let channel_slug_present", "pricing channel slug presence"],
  ["let channel_slug_length", "pricing channel slug length"],
  ["let locale_present", "pricing locale presence"],
  ["let locale_length", "pricing locale length"],
  ["correlation_id = ?correlation_id", "pricing correlation diagnostic"],
  ["request_tenant_id_non_nil = ?request_tenant_id_non_nil", "pricing request tenant diagnostic"],
  ["tenant_id_non_nil", "pricing tenant diagnostic"],
  ["cart_id_non_nil", "pricing cart diagnostic"],
  ["line_item_id_non_nil", "pricing line-item diagnostic"],
  ["channel_id_non_nil = ?channel_id_non_nil", "pricing channel diagnostic"],
  ["channel_slug_length = ?channel_slug_length", "pricing slug diagnostic"],
  ["locale_length = ?locale_length", "pricing locale diagnostic"],
  ["owner_code = %error.code", "pricing owner code diagnostic"],
  ["owner_kind = ?error.kind", "pricing owner kind diagnostic"],
  ["owner_retryable = error.retryable", "pricing owner retryability diagnostic"],
  ["cart storefront pricing operation failed", "pricing technical message"],
  ["cart storefront pricing operation was rejected", "pricing rejection message"],
  ["ServerFnError::new(error.message)", "pricing sanitized public envelope"],
]) requireText(pricingBody, value, label);
for (const payload of [
  "error = ?error",
  "error = %error",
  "request_tenant_id = ?request_context.map",
  "tenant_id = %tenant_id",
  "cart_id = %cart_id",
  "cart_id = ?cart_id",
  "line_item_id = %line_item_id",
  "line_item_id = ?line_item_id",
  "channel_id = ?request_context",
  "channel_slug = ?request_context",
  "locale = ?request_context",
]) forbidText(pricingBody, payload, "complete pricing cause or raw identity/context diagnostic");
if (countText(pricingBody, "let error_type = std::any::type_name_of_val(&error);") !== 1) {
  failures.push("expected exactly one pricing type-only diagnostic site");
}
if (countText(pricingBody, "tracing::error!(") !== 1) {
  failures.push("expected exactly one pricing technical diagnostic path");
}
if (countText(pricingBody, "tracing::warn!(") !== 1) {
  failures.push("expected exactly one pricing rejection diagnostic path");
}
for (const marker of [
  "correlation_id = ?correlation_id",
  "owner_code = %error.code",
  "owner_kind = ?error.kind",
  "owner_retryable = error.retryable",
]) {
  if (countText(pricingBody, marker) !== 2) {
    failures.push(`both pricing severity paths must retain ${marker}`);
  }
}
if (countText(safe, "let error_type = std::any::type_name_of_val(&error);") !== 3) {
  failures.push("expected customer, Cart owner, and pricing type-only diagnostic sites");
}
if (countText(safe, "error = ?error") !== 1) {
  failures.push(
    "only the Cart input diagnostic may retain a complete error in this bounded pricing-only slice",
  );
}

const missingVariantBody = functionBody(safe, "missing_variant_error");
for (const marker of [
  "tenant_id = %tenant_id",
  "cart_id = %cart_id",
  "line_item_id = %line_item_id",
  'code = "cart.storefront_line_item_variant_missing"',
  'ServerFnError::new("Cart line item could not be updated safely")',
]) requireText(missingVariantBody, marker, "unchanged missing-variant diagnostic boundary");

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
  cart_static_public_envelopes: true,
  cart_error_type_only: true,
  complete_cart_error_logged: false,
  cart_correlation_diagnostic: true,
  cart_identity_shape_only: true,
  cart_context_shape_only: true,
  raw_cart_identity_context_logged: false,
  cart_public_mapping_preserved: true,
  cart_severity_split_preserved: true,
  cart_raw_error_public: false,
  pricing_error_type_only: true,
  complete_pricing_error_logged: false,
  pricing_correlation_diagnostic: true,
  pricing_identity_shape_only: true,
  pricing_context_shape_only: true,
  raw_pricing_identity_context_logged: false,
  pricing_owner_metadata_preserved: true,
  pricing_severity_split_preserved: true,
  pricing_port_message_remains_domain_sanitized: true,
  owner_context_logging: true,
  input_identifier_mapper_cleanup_out_of_scope: true,
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
  "Cart owner failures are recorded by concrete error type",
  "documentation Cart owner diagnostic policy",
);
requireText(
  doc,
  "Pricing failures are recorded by concrete error type",
  "documentation pricing diagnostic policy",
);
requireText(
  doc,
  "Cart input and missing-variant diagnostics remain separate open cleanup slices",
  "documentation remaining mapper boundary",
);

if (failures.length > 0) {
  console.error("Cart storefront native error-safety verification failed:");
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ cart storefront framework, customer, Cart owner, and pricing diagnostics use bounded causes/context while input and identifier cleanup remains open; source evidence remains unvalidated",
);
