#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { forbidRawContextLogging } from "./lib/bounded-diagnostics.mjs";

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL("../../", import.meta.url);
const read = (relativePath) =>
  readFileSync(new URL(relativePath, root), "utf8");
const moduleSource = read(
  "crates/modules/rustok-commerce/src/graphql/mutations/mod.rs",
);
const facadeSource = read(
  "crates/modules/rustok-commerce/src/graphql/mutations/safe_helpers.rs",
);
const legacySource = read(
  "crates/modules/rustok-commerce/src/graphql/mutations/helpers.rs",
);
const layeredSource = read(
  "crates/modules/rustok-commerce/src/graphql/mutations/layered_order_helpers.rs",
);
const mountedSource = read(
  "crates/modules/rustok-commerce/src/graphql/mutations/cart.rs",
);
const workflowSource = read(
  "crates/modules/rustok-commerce/src/storefront_cart_runtime.rs",
);
const failures = [];

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const between = (source, start, end, label) => {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return "";
  }
  return source.slice(startIndex, endIndex);
};

const customerMapper = between(
  facadeSource,
  "fn customer_port_graphql_error(",
  "fn cart_port_source_owner(",
  "customer GraphQL mapper",
);
const customerLookup = between(
  facadeSource,
  "pub(crate) async fn resolve_optional_storefront_customer_id(",
  "fn legacy_graphql_error(",
  "optional storefront customer lookup",
);
const legacyMapper = between(
  facadeSource,
  "fn legacy_graphql_error(",
  "pub(crate) async fn enrich_storefront_cart(",
  "legacy GraphQL mapper",
);
const dependencyMapper = between(
  workflowSource,
  "fn dependency_error(",
  "#[cfg(test)]",
  "shared line-item dependency mapper",
);
for (const [value, label] of [
  [
    '#[path = "safe_helpers.rs"]\nmod cart_safe_helpers;',
    "private cart safe facade routing",
  ],
  [
    '#[path = "safe_order_helpers.rs"]\nmod safe_order_helpers_impl;',
    "private order helper implementation routing",
  ],
  [
    '#[path = "layered_order_helpers.rs"]\npub mod helpers;',
    "public layered helper routing",
  ],
  [
    '#[path = "safe_legacy_helpers.rs"]\nmod legacy_helpers;',
    "private legacy helper routing",
  ],
  [
    "pub(crate) use super::safe_order_helpers_impl::*;",
    "preserved helper symbol parity",
  ],
]) {
  requireText(`${moduleSource}\n${layeredSource}`, value, label);
}

for (const value of [
  "async_graphql::Error::new(error.message)",
  "async_graphql::Error::new(error.to_string())",
  'async_graphql::Error::new(format!("{error}"))',
  "pub use super::legacy_helpers::*;",
  'format!("{error:?}")',
  "detail.contains(",
  "super::legacy_helpers::resolve_storefront_line_item_input(",
  "super::legacy_helpers::validate_storefront_line_item_quantity(",
  "Variant not found",
  "Product not found",
  "does not have enough available inventory",
  "Invalid JSON metadata payload",
]) {
  forbidText(
    facadeSource,
    value,
    "storefront cart compatibility safe helper facade",
  );
}

for (const [value, label] of [
  ["PortActorKind", "actor kind import"],
  ["PortError, PortErrorKind", "port error imports"],
  [
    'const STOREFRONT_CART_HELPER_BOUNDARY: &str = "commerce_graphql_storefront_cart_helper";',
    "shared GraphQL boundary",
  ],
  [
    'const STOREFRONT_CUSTOMER_OWNER: &str = "rustok_customer";',
    "truthful customer owner",
  ],
  [
    'const STOREFRONT_CUSTOMER_OWNER_OPERATION: &str = "read_customer_projection_by_user";',
    "exact customer owner operation",
  ],
  ["struct StorefrontCustomerDiagnosticContext", "bounded customer context"],
  [
    "impl From<&PortContext> for StorefrontCustomerDiagnosticContext",
    "customer context projection",
  ],
  [
    "tenant_id_shape: identity_text_shape(context.tenant_id.as_str())",
    "tenant identity shape",
  ],
  ["actor_kind: actor_kind_name(&context.actor.kind)", "actor kind projection"],
  [
    "actor_id_shape: identity_text_shape(context.actor.id.as_str())",
    "actor identity shape",
  ],
  ["claim_count: context.claims.len()", "claim count"],
  ["role_count: context.roles.len()", "role count"],
  [
    "channel_shape: optional_text_shape(context.channel.as_deref())",
    "channel shape",
  ],
  ["locale_shape: text_shape(context.locale.as_str())", "locale shape"],
  [
    "correlation_id_shape: text_shape(context.correlation_id.as_str())",
    "correlation shape",
  ],
  [
    "causation_id_shape: optional_text_shape(context.causation_id.as_deref())",
    "causation shape",
  ],
  [
    "traceparent_shape: optional_text_shape(context.traceparent.as_deref())",
    "trace shape",
  ],
  [
    "idempotency_key_shape: optional_text_shape(context.idempotency_key.as_deref())",
    "idempotency shape",
  ],
  ["deadline_ms: context.deadline_ms", "deadline fact"],
  [
    "struct StorefrontCustomerDiagnosticError;",
    "redacted customer diagnostic error",
  ],
  [
    "struct StorefrontLegacyGraphqlDiagnosticError;",
    "redacted legacy diagnostic error",
  ],
  [
    "impl From<async_graphql::Error> for StorefrontLegacyGraphqlDiagnosticError",
    "legacy error consumption",
  ],
  ['formatter.write_str("redacted")', "redacted diagnostic Debug"],
  ["fn actor_kind_name(kind: &PortActorKind)", "actor kind helper"],
  ["fn identity_text_shape(value: &str)", "identity shape helper"],
  ["fn uuid_shape(value: Uuid)", "UUID shape helper"],
  ["fn optional_uuid_shape(value: Option<Uuid>)", "optional UUID shape helper"],
  ["fn text_shape(value: &str)", "text shape helper"],
  ["fn optional_text_shape(value: Option<&str>)", "optional text shape helper"],
  ["fn customer_port_graphql_error(", "customer mapper"],
  ["fn cart_port_source_owner(", "cart source-owner classifier"],
  ["pub(crate) fn cart_port_error(", "cart mapper"],
  ['Some(("cart", _)) => "rustok_cart"', "cart owner classification"],
  ['Some(("pricing", _)) => "rustok_pricing"', "pricing owner classification"],
  ['_ => "unknown"', "unknown owner classification"],
  [
    'owner = "rustok_commerce.graphql_cart_helper"',
    "commerce boundary owner logging",
  ],
  [
    "let source_owner = cart_port_source_owner(&error);",
    "typed source owner projection",
  ],
  ["source_owner,", "typed source owner logging"],
  ["owner_code = %error.code", "cart owner code logging"],
  ["owner_kind = ?error.kind", "cart owner kind logging"],
  ['error_kind = "legacy_graphql_error"', "legacy error kind logging"],
  ["tenant_id_shape,", "tenant shape logging"],
  ["resource_id_shape,", "resource shape logging"],
  [
    '"Cart shipping details are temporarily unavailable"',
    "cart enrichment message",
  ],
  ['"Selected shipping option is invalid"', "shipping selection message"],
  ['"Cart pricing could not be refreshed"', "reprice fallback message"],
  ['extensions.set("retryable", retryable)', "retryability extension"],
]) {
  requireText(facadeSource, value, label);
}

for (const [value, label] of [
  ["context: &PortContext", "retained customer context input"],
  ["consumer_operation: &'static str", "consumer operation input"],
  ["error: PortError", "original customer error input"],
  ["PortErrorKind::Validation", "customer validation mapping"],
  ["PortErrorKind::NotFound", "customer not-found mapping"],
  ["PortErrorKind::Conflict", "customer conflict mapping"],
  ["PortErrorKind::Forbidden", "customer forbidden mapping"],
  [
    "PortErrorKind::Unavailable | PortErrorKind::Timeout",
    "customer availability mapping",
  ],
  ["PortErrorKind::InvariantViolation", "customer invariant mapping"],
  ['"CUSTOMER_REQUEST_INVALID"', "customer validation code"],
  ['"CUSTOMER_NOT_FOUND"', "customer not-found code"],
  ['"CUSTOMER_STATE_CONFLICT"', "customer conflict code"],
  ['"CUSTOMER_ACCESS_DENIED"', "customer forbidden code"],
  ['"CUSTOMER_TEMPORARILY_UNAVAILABLE"', "customer availability code"],
  ['"CUSTOMER_OPERATION_FAILED"', "customer invariant code"],
  ["let technical = matches!(", "technical severity selection"],
  [
    "PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation",
    "technical customer severity classification",
  ],
  [
    "let diagnostic_context = StorefrontCustomerDiagnosticContext::from(context);",
    "bounded customer context projection",
  ],
  ["let owner_code = error.code.clone();", "retained owner code"],
  ["let owner_kind = error.kind.clone();", "retained owner kind"],
  ["let owner_retryable = error.retryable;", "retained owner retryability"],
  [
    "let owner_message_shape = text_shape(error.message.as_str());",
    "owner message shape",
  ],
  ["let owner_message_len = error.message.len();", "owner message length"],
  [
    "let error = StorefrontCustomerDiagnosticError;",
    "customer diagnostic shadow",
  ],
  ["if technical {", "technical branch"],
  ["tracing::error!(", "technical customer error severity"],
  ["tracing::warn!(", "ordinary customer rejection severity"],
  ["error = ?error", "redacted customer error field"],
  ["owner = STOREFRONT_CUSTOMER_OWNER", "truthful customer owner field"],
  [
    "owner_operation = STOREFRONT_CUSTOMER_OWNER_OPERATION",
    "exact owner operation field",
  ],
  ["consumer_operation,", "consumer operation field"],
  ["tenant_id_shape = diagnostic_context.tenant_id_shape", "tenant shape log"],
  ["actor_kind = diagnostic_context.actor_kind", "actor kind log"],
  [
    "actor_id_shape = diagnostic_context.actor_id_shape",
    "actor identity shape log",
  ],
  ["claim_count = diagnostic_context.claim_count", "claim count log"],
  ["role_count = diagnostic_context.role_count", "role count log"],
  ["channel_shape = diagnostic_context.channel_shape", "channel shape log"],
  ["locale_shape = diagnostic_context.locale_shape", "locale shape log"],
  [
    "correlation_id_shape = diagnostic_context.correlation_id_shape",
    "correlation shape log",
  ],
  [
    "causation_id_shape = diagnostic_context.causation_id_shape",
    "causation shape log",
  ],
  [
    "traceparent_shape = diagnostic_context.traceparent_shape",
    "trace shape log",
  ],
  [
    "idempotency_key_shape = diagnostic_context.idempotency_key_shape",
    "idempotency shape log",
  ],
  ["deadline_ms = ?diagnostic_context.deadline_ms", "customer deadline fact"],
  ["owner_code = %owner_code", "customer owner code"],
  ["owner_message_shape,", "customer owner message shape"],
  ["owner_message_len,", "customer owner message length"],
  ["owner_kind = ?owner_kind", "customer typed owner kind"],
  ["owner_retryable,", "customer owner retryability"],
  ["public_code = code", "customer public code"],
  ["public_retryable = retryable", "customer public retryability"],
  ["boundary = STOREFRONT_CART_HELPER_BOUNDARY", "customer GraphQL boundary"],
  [
    '"commerce GraphQL storefront customer owner port failed"',
    "technical customer event",
  ],
  [
    '"commerce GraphQL storefront customer owner port was rejected"',
    "ordinary customer rejection event",
  ],
  [
    "public_graphql_error(message, code, retryable)",
    "unchanged safe customer envelope",
  ],
]) {
  requireText(customerMapper, value, label);
}

for (const value of [
  "correlation_id = %context.correlation_id",
  "tenant_id = %context.tenant_id",
  "actor = ?context.actor",
  "channel = ?context.channel",
  "locale = %context.locale",
  "causation_id = ?context.causation_id",
  "traceparent = ?context.traceparent",
  "idempotency_key = ?context.idempotency_key",
  "internal_code = %error.code",
  "internal_message = %error.message",
  "owner_kind = ?error.kind",
  "            owner_retryable = error.retryable,",
]) {
  forbidText(customerMapper, value, "raw customer diagnostic");
}

const policyIndex = customerMapper.indexOf(
  "let (message, code, retryable) = match &error.kind",
);
const projectionIndex = customerMapper.indexOf(
  "let diagnostic_context = StorefrontCustomerDiagnosticContext::from(context);",
);
const shadowIndex = customerMapper.indexOf(
  "let error = StorefrontCustomerDiagnosticError;",
);
const diagnosticIndex = customerMapper.indexOf("tracing::error!(");
const returnIndex = customerMapper.lastIndexOf(
  "public_graphql_error(message, code, retryable)",
);
if (
  !(
    policyIndex >= 0 &&
    policyIndex < projectionIndex &&
    projectionIndex < shadowIndex &&
    shadowIndex < diagnosticIndex &&
    diagnosticIndex < returnIndex
  )
) {
  failures.push(
    "customer error must be mapped, projected, shadowed, diagnosed, and returned in order",
  );
}
if (
  (
    customerMapper.match(/let error = StorefrontCustomerDiagnosticError;/g) ??
    []
  ).length !== 1
) {
  failures.push("expected one customer diagnostic shadow");
}
if ((customerMapper.match(/error = \?error/g) ?? []).length !== 2) {
  failures.push("expected two redacted customer diagnostic error fields");
}

for (const [value, label] of [
  [
    "let customer_context = storefront_customer_port_context(tenant_id, auth.user_id);",
    "single retained customer context",
  ],
  ["read_customer_projection_by_user(", "customer owner call"],
  ["customer_context.clone(),", "customer context delegation clone"],
  ["CustomerUserProjectionRequest {", "customer projection request"],
  ["user_id: auth.user_id,", "customer user identity"],
  [
    'Err(error) if error.code == "customer.customer_by_user_not_found" => Ok(None)',
    "unchanged optional-customer fallback",
  ],
  ["&customer_context,", "retained customer mapper context"],
  ['"resolve_optional_storefront_customer_id"', "consumer operation value"],
]) {
  requireText(customerLookup, value, label);
}

for (const [value, label] of [
  ["error: async_graphql::Error", "original legacy GraphQL error input"],
  ["tenant_id: Uuid", "legacy tenant input"],
  ["resource_id: Option<Uuid>", "legacy resource input"],
  ["operation: &'static str", "legacy operation input"],
  ["message: &'static str", "legacy public message input"],
  ["code: &'static str", "legacy public code input"],
  ["retryable: bool", "legacy public retryability input"],
  ["let tenant_id_shape = uuid_shape(tenant_id);", "legacy tenant projection"],
  [
    "let resource_id_shape = optional_uuid_shape(resource_id);",
    "legacy resource projection",
  ],
  [
    "let error = StorefrontLegacyGraphqlDiagnosticError::from(error);",
    "legacy error consumption and shadow",
  ],
  ["tracing::error!(", "legacy error severity"],
  ["error = ?error", "redacted legacy error field"],
  ['owner = "rustok_commerce.graphql_cart_helper"', "legacy boundary owner"],
  ["tenant_id_shape,", "legacy tenant shape field"],
  ["resource_id_shape,", "legacy resource shape field"],
  ["operation,", "legacy operation field"],
  ['error_kind = "legacy_graphql_error"', "legacy error kind field"],
  ["public_code = code", "legacy public code field"],
  ["public_retryable = retryable", "legacy public retryability field"],
  ["boundary = STOREFRONT_CART_HELPER_BOUNDARY", "legacy shared boundary"],
  ['"commerce GraphQL storefront cart helper failed"', "legacy static event"],
  [
    "public_graphql_error(message, code, retryable)",
    "legacy stable public envelope",
  ],
]) {
  requireText(legacyMapper, value, label);
}

for (const value of [
  "tenant_id = %tenant_id",
  "tenant_id = ?tenant_id",
  "resource_id = ?resource_id",
  "resource_id = %resource_id",
  'format!("{error:?}")',
  "error.to_string()",
  "error.message",
]) {
  forbidText(legacyMapper, value, "raw legacy GraphQL diagnostic");
}

const legacyTenantProjectionIndex = legacyMapper.indexOf(
  "let tenant_id_shape = uuid_shape(tenant_id);",
);
const legacyResourceProjectionIndex = legacyMapper.indexOf(
  "let resource_id_shape = optional_uuid_shape(resource_id);",
);
const legacyShadowIndex = legacyMapper.indexOf(
  "let error = StorefrontLegacyGraphqlDiagnosticError::from(error);",
);
const legacyEventIndex = legacyMapper.indexOf("tracing::error!(");
const legacyReturnIndex = legacyMapper.lastIndexOf(
  "public_graphql_error(message, code, retryable)",
);
if (
  !(
    legacyTenantProjectionIndex >= 0 &&
    legacyTenantProjectionIndex < legacyResourceProjectionIndex &&
    legacyResourceProjectionIndex < legacyShadowIndex &&
    legacyShadowIndex < legacyEventIndex &&
    legacyEventIndex < legacyReturnIndex
  )
) {
  failures.push(
    "legacy error must be projected, consumed, diagnosed, and returned in order",
  );
}
if ((legacyMapper.match(/tracing::error!\(/g) ?? []).length !== 1) {
  failures.push("expected one legacy GraphQL diagnostic event");
}
if ((legacyMapper.match(/error = \?error/g) ?? []).length !== 1) {
  failures.push("expected one redacted legacy GraphQL error field");
}

for (const operation of [
  "resolve_optional_storefront_customer_id",
  "enrich_storefront_cart",
  "validate_selected_shipping_option",
  "reprice_storefront_cart_line_items",
]) {
  requireText(
    facadeSource,
    `"${operation}"`,
    `${operation} compatibility operation mapping`,
  );
}
const legacyCalls =
  facadeSource.match(/super::legacy_helpers::[a-z_]+\(/g) ?? [];
if (legacyCalls.length !== 2) {
  failures.push(
    `expected 2 remaining compatibility legacy helper calls, found ${legacyCalls.length}`,
  );
}

for (const source of [moduleSource, facadeSource, legacySource, layeredSource, mountedSource, workflowSource]) {
  for (const retired of [
    "typed_line_item_helpers",
    "resolve_storefront_line_item_input",
    "validate_storefront_line_item_quantity",
    "rustok_product::entities",
  ]) {
    forbidText(source, retired, "retired cart line-item path");
  }
}

for (const [resolver, nextResolver, operation] of [
  ["add_storefront_cart_line_item", "update_storefront_cart_context", "add_line_item"],
  ["update_storefront_cart_line_item", "remove_storefront_cart_line_item", "update_line_item_quantity"],
]) {
  const body = between(mountedSource, `async fn ${resolver}(`, `async fn ${nextResolver}(`, resolver);
  requireText(body, `.${operation}(`, resolver);
  requireText(body, ".map_err(cart_port_error)?", resolver);
}

for (const [start, end, markers] of [
  ["pub async fn add_line_item(", "pub async fn update_line_item_quantity(", [
    ".resolve_product_price(", ".add_storefront_line_item(",
  ]],
  ["pub async fn update_line_item_quantity(", "fn require_active_cart(", [
    "crate::read_storefront_cart_line_quote(", "pricing_context,", "CartStorefrontLineItemPricingRequest",
  ]],
]) {
  const body = between(workflowSource, start, end, "shared cart workflow");
  for (const marker of [".read_storefront_variant_product_projection(", "require_cart_inventory(", ...markers]) {
    requireText(body, marker, start);
  }
}

for (const owner of ["cart", "pricing", "product", "inventory"]) {
  requireText(workflowSource, `dependency_error("${owner}", error)`, "typed owner failure");
}
for (const value of ["kind = ?error.kind", "retryable = error.retryable", "code_length = error.code.len()", 'format!("commerce.cart_{owner}_refused")', "error.retryable,"]) {
  requireText(dependencyMapper, value, "bounded line-item dependency mapper");
}
for (const value of ["error.message", "error.to_string(", "?error,", "%error,", "%error.code", "?error.code"]) {
  forbidText(dependencyMapper, value, "bounded line-item dependency mapper");
}
forbidRawContextLogging(failures, dependencyMapper, "shared line-item dependency mapper");

if (failures.length > 0) {
  console.error(
    "Commerce GraphQL storefront cart helper error safety verification failed:",
  );
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  "✔ Commerce GraphQL storefront cart helpers keep stable envelopes, bounded diagnostics, typed line-item outcomes, and private layered routing",
);
