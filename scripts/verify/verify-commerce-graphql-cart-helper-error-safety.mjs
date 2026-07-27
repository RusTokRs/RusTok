#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const moduleSource = read('crates/rustok-commerce/src/graphql/mutations/mod.rs');
const facadeSource = read('crates/rustok-commerce/src/graphql/mutations/safe_helpers.rs');
const typedSource = read(
  'crates/rustok-commerce/src/graphql/mutations/typed_line_item_helpers.rs',
);
const layeredSource = read(
  'crates/rustok-commerce/src/graphql/mutations/layered_order_helpers.rs',
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
    return '';
  }
  return source.slice(startIndex, endIndex);
};

const customerMapper = between(
  facadeSource,
  'fn customer_port_graphql_error(',
  'fn cart_port_source_owner(',
  'customer GraphQL mapper',
);
const customerLookup = between(
  facadeSource,
  'pub(crate) async fn resolve_optional_storefront_customer_id(',
  'fn legacy_graphql_error(',
  'optional storefront customer lookup',
);
const typedPolicy = between(
  typedSource,
  'fn storefront_line_item_public_policy(',
  '#[allow(clippy::too_many_arguments)]\nfn storefront_line_item_graphql_error(',
  'typed line item public policy',
);
const typedMapper = between(
  typedSource,
  'fn storefront_line_item_graphql_error(',
  'fn parse_line_item_metadata(',
  'typed line item GraphQL mapper',
);

for (const [value, label] of [
  ['#[path = "safe_helpers.rs"]\nmod cart_safe_helpers;', 'private cart safe facade routing'],
  [
    '#[path = "typed_line_item_helpers.rs"]\nmod typed_line_item_helpers;',
    'private typed line item routing',
  ],
  [
    '#[path = "safe_order_helpers.rs"]\nmod safe_order_helpers_impl;',
    'private order helper implementation routing',
  ],
  [
    '#[path = "layered_order_helpers.rs"]\npub mod helpers;',
    'public layered helper routing',
  ],
  ['#[path = "safe_legacy_helpers.rs"]\nmod legacy_helpers;', 'private legacy helper routing'],
  ['pub(crate) use super::safe_order_helpers_impl::*;', 'preserved helper symbol parity'],
  [
    'resolve_storefront_line_item_input, validate_storefront_line_item_quantity,',
    'explicit typed line item overrides',
  ],
]) {
  requireText(`${moduleSource}\n${layeredSource}`, value, label);
}

for (const value of [
  'async_graphql::Error::new(error.message)',
  'async_graphql::Error::new(error.to_string())',
  'async_graphql::Error::new(format!("{error}"))',
  'pub use super::legacy_helpers::*;',
]) {
  forbidText(facadeSource, value, 'storefront cart compatibility safe helper facade');
}

for (const [value, label] of [
  ['PortError, PortErrorKind', 'port error imports'],
  [
    'const STOREFRONT_CART_HELPER_BOUNDARY: &str = "commerce_graphql_storefront_cart_helper";',
    'shared GraphQL boundary',
  ],
  ['const STOREFRONT_CUSTOMER_OWNER: &str = "rustok_customer";', 'truthful customer owner'],
  [
    'const STOREFRONT_CUSTOMER_OWNER_OPERATION: &str = "read_customer_projection_by_user";',
    'exact customer owner operation',
  ],
  ['fn customer_port_graphql_error(', 'customer mapper'],
  ['fn cart_port_source_owner(', 'cart source-owner classifier'],
  ['pub(crate) fn cart_port_error(', 'cart mapper'],
  ['Some(("cart", _)) => "rustok_cart"', 'cart owner classification'],
  ['Some(("pricing", _)) => "rustok_pricing"', 'pricing owner classification'],
  ['_ => "unknown"', 'unknown owner classification'],
  ['owner = "rustok_commerce.graphql_cart_helper"', 'commerce boundary owner logging'],
  ['source_owner = cart_port_source_owner(&error)', 'typed source owner logging'],
  ['owner_code = %error.code', 'cart owner code logging'],
  ['owner_kind = ?error.kind', 'cart owner kind logging'],
  ['error_kind = "legacy_graphql_error"', 'legacy error kind logging'],
  ['resource_id = ?resource_id', 'resource logging'],
  ['"Cart shipping details are temporarily unavailable"', 'cart enrichment message'],
  ['"Selected shipping option is invalid"', 'shipping selection message'],
  ['"Cart pricing could not be refreshed"', 'reprice fallback message'],
  ['extensions.set("retryable", retryable)', 'retryability extension'],
]) {
  requireText(facadeSource, value, label);
}

for (const [value, label] of [
  ['context: &PortContext', 'retained customer context input'],
  ["consumer_operation: &'static str", 'consumer operation input'],
  ['error: PortError', 'original customer error input'],
  ['PortErrorKind::Validation', 'customer validation mapping'],
  ['PortErrorKind::NotFound', 'customer not-found mapping'],
  ['PortErrorKind::Conflict', 'customer conflict mapping'],
  ['PortErrorKind::Forbidden', 'customer forbidden mapping'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'customer availability mapping'],
  ['PortErrorKind::InvariantViolation', 'customer invariant mapping'],
  ['"CUSTOMER_REQUEST_INVALID"', 'customer validation code'],
  ['"CUSTOMER_NOT_FOUND"', 'customer not-found code'],
  ['"CUSTOMER_STATE_CONFLICT"', 'customer conflict code'],
  ['"CUSTOMER_ACCESS_DENIED"', 'customer forbidden code'],
  ['"CUSTOMER_TEMPORARILY_UNAVAILABLE"', 'customer availability code'],
  ['"CUSTOMER_OPERATION_FAILED"', 'customer invariant code'],
  [
    'PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation',
    'technical customer severity classification',
  ],
  ['tracing::error!(', 'technical customer error severity'],
  ['tracing::warn!(', 'ordinary customer rejection severity'],
  ['error = ?error', 'original customer error evidence'],
  ['owner = STOREFRONT_CUSTOMER_OWNER', 'truthful customer owner field'],
  ['owner_operation = STOREFRONT_CUSTOMER_OWNER_OPERATION', 'exact owner operation field'],
  ['consumer_operation,', 'consumer operation field'],
  ['correlation_id = %context.correlation_id', 'customer correlation context'],
  ['tenant_id = %context.tenant_id', 'customer tenant context'],
  ['actor = ?context.actor', 'customer actor context'],
  ['channel = ?context.channel', 'customer channel context'],
  ['locale = %context.locale', 'customer locale context'],
  ['causation_id = ?context.causation_id', 'customer causation context'],
  ['traceparent = ?context.traceparent', 'customer trace context'],
  ['idempotency_key = ?context.idempotency_key', 'customer idempotency context'],
  ['deadline_ms = ?context.deadline_ms', 'customer deadline context'],
  ['internal_code = %error.code', 'customer internal code'],
  ['internal_message = %error.message', 'customer internal message'],
  ['error_kind = ?error.kind', 'customer typed error kind'],
  ['owner_retryable = error.retryable', 'customer owner retryability'],
  ['public_code = code', 'customer public code'],
  ['public_retryable = retryable', 'customer public retryability'],
  ['boundary = STOREFRONT_CART_HELPER_BOUNDARY', 'customer GraphQL boundary'],
  ['"commerce GraphQL storefront customer owner port failed"', 'technical customer event'],
  [
    '"commerce GraphQL storefront customer owner port was rejected"',
    'ordinary customer rejection event',
  ],
  ['public_graphql_error(message, code, retryable)', 'unchanged safe customer envelope'],
]) {
  requireText(customerMapper, value, label);
}

const policyIndex = customerMapper.indexOf('let (message, code, retryable) = match &error.kind');
const diagnosticsIndex = customerMapper.indexOf('match &error.kind', policyIndex + 1);
const returnIndex = customerMapper.lastIndexOf('public_graphql_error(message, code, retryable)');
if (!(policyIndex >= 0 && policyIndex < diagnosticsIndex && diagnosticsIndex < returnIndex)) {
  failures.push('customer error must be mapped, diagnosed, and then returned in order');
}

for (const [value, label] of [
  [
    'let customer_context = storefront_customer_port_context(tenant_id, auth.user_id);',
    'single retained customer context',
  ],
  ['read_customer_projection_by_user(', 'customer owner call'],
  ['customer_context.clone(),', 'customer context delegation clone'],
  ['CustomerUserProjectionRequest {', 'customer projection request'],
  ['user_id: auth.user_id,', 'customer user identity'],
  [
    'Err(error) if error.code == "customer.customer_by_user_not_found" => Ok(None)',
    'unchanged optional-customer fallback',
  ],
  ['&customer_context,', 'retained customer mapper context'],
  ['"resolve_optional_storefront_customer_id"', 'consumer operation value'],
]) {
  requireText(customerLookup, value, label);
}

for (const [value, label] of [
  ['enum StorefrontLineItemFailureKind {', 'typed failure kind'],
  ['ProductUnavailable,', 'typed product outcome'],
  ['InventoryInsufficient,', 'typed inventory outcome'],
  ['InputInvalid,', 'typed input outcome'],
  ['DependencyUnavailable,', 'typed dependency outcome'],
  ['enum StorefrontLineItemFailureSource {', 'typed source cause'],
  ['Database(sea_orm::DbErr)', 'typed database cause'],
  ['Pricing(PortError)', 'typed pricing cause'],
  ['Inventory(CommerceError)', 'typed inventory cause'],
  ['Metadata(serde_json::Error)', 'typed metadata cause'],
  ['fn storefront_line_item_public_policy(', 'typed public policy'],
  ['fn storefront_line_item_graphql_error(', 'typed GraphQL mapper'],
  ['async fn resolve_typed_storefront_line_item_input(', 'typed resolver'],
  ['async fn validate_typed_storefront_line_item_quantity(', 'typed quantity validator'],
  ['async fn validate_typed_storefront_variant_inventory(', 'typed inventory validator'],
  ['StorefrontLineItemFailure::database("load_variant", error)', 'variant database mapping'],
  ['StorefrontLineItemFailure::pricing(product_model.id, error)', 'pricing mapping'],
  ['StorefrontLineItemFailure::inventory(variant.product_id, error)', 'inventory mapping'],
  ['StorefrontLineItemFailure::invalid_metadata(product_id, error)', 'metadata mapping'],
  ['StorefrontLineItemFailure::inventory_insufficient(', 'inventory insufficiency mapping'],
  ['resolve_typed_storefront_line_item_input(', 'mounted typed resolver delegation'],
  ['validate_typed_storefront_line_item_quantity(', 'mounted typed quantity delegation'],
]) {
  requireText(typedSource, value, label);
}

for (const [value, label] of [
  ['"Product is not available"', 'product public message'],
  ['"CART_PRODUCT_UNAVAILABLE"', 'product public code'],
  ['"Requested quantity is not available"', 'inventory public message'],
  ['"CART_INVENTORY_INSUFFICIENT"', 'inventory public code'],
  ['"Cart line item input is invalid"', 'input public message'],
  ['"CART_LINE_ITEM_INVALID"', 'input public code'],
  ['"Cart line item could not be resolved"', 'resolver fallback message'],
  ['"CART_LINE_ITEM_RESOLUTION_FAILED"', 'resolver fallback code'],
  ['"Inventory availability could not be verified"', 'quantity fallback message'],
  ['"CART_INVENTORY_UNAVAILABLE"', 'quantity fallback code'],
]) {
  requireText(typedPolicy, value, label);
}

for (const [value, label] of [
  ['source = ?source', 'typed source diagnostics'],
  ['source_kind,', 'typed source-kind diagnostics'],
  ['owner = failure.source_owner', 'truthful source owner'],
  ['owner_operation = failure.source_operation', 'truthful source operation'],
  ['consumer_operation = consumer_operation.name()', 'consumer operation'],
  ['correlation_id = ?correlation_id', 'optional retained correlation'],
  ['tenant_id = %tenant_id', 'tenant context'],
  ['variant_id = %variant_id', 'variant context'],
  ['product_id = ?failure.product_id', 'product context'],
  ['requested_quantity,', 'quantity context'],
  ['channel_slug_length = ?channel_slug_length', 'bounded channel fact'],
  ['locale_length = ?locale_length', 'bounded locale fact'],
  ['public_code = code', 'public code diagnostics'],
  ['public_retryable = retryable', 'public retryability diagnostics'],
  ['boundary = STOREFRONT_LINE_ITEM_GRAPHQL_BOUNDARY', 'typed line item boundary'],
  ['tracing::error!(', 'dependency error severity'],
  ['tracing::warn!(', 'ordinary rejection severity'],
  ['public_graphql_error(message, code, retryable)', 'stable public envelope'],
]) {
  requireText(typedMapper, value, label);
}

for (const value of [
  'format!("{error:?}")',
  'detail.contains(',
  'async_graphql::Error::new(error.to_string())',
  'async_graphql::Error::new(format!("{error}"))',
  'error.message',
  'public_channel_slug = ?public_channel_slug',
  'locale = ?locale',
  'metadata = ?input.metadata',
  'sku = %',
  'sku = ?',
  'title = %',
  'title = ?',
]) {
  forbidText(typedSource, value, 'typed storefront line item boundary');
}

for (const operation of [
  'resolve_optional_storefront_customer_id',
  'enrich_storefront_cart',
  'validate_selected_shipping_option',
  'reprice_storefront_cart_line_items',
]) {
  requireText(facadeSource, `"${operation}"`, `${operation} compatibility operation mapping`);
}
for (const operation of [
  'resolve_storefront_line_item_input',
  'validate_storefront_line_item_quantity',
]) {
  requireText(typedSource, `"${operation}"`, `${operation} typed operation mapping`);
}

const legacyCalls = facadeSource.match(/super::legacy_helpers::[a-z_]+\(/g) ?? [];
if (legacyCalls.length !== 5) {
  failures.push(`expected 5 compatibility legacy helper calls, found ${legacyCalls.length}`);
}
const typedExports = layeredSource.match(
  /resolve_storefront_line_item_input|validate_storefront_line_item_quantity/g,
) ?? [];
if (typedExports.length !== 2) {
  failures.push(`expected 2 explicit typed helper exports, found ${typedExports.length}`);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL storefront cart helper error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL storefront cart helpers use typed line-item outcomes, stable public envelopes, retained owner context, and private layered routing',
);
