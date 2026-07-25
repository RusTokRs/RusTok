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
const facadeSource = read('crates/rustok-commerce/src/graphql/mutations/safe_order_helpers.rs');
const orderErrors = read('crates/rustok-order/src/error.rs');
const commerceErrors = read('crates/rustok-commerce-foundation/src/error.rs');
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

const orderPolicy = between(
  facadeSource,
  'fn storefront_order_graphql_error_policy(',
  'fn order_graphql_error(',
  'order GraphQL policy',
);
const orderMapper = between(
  facadeSource,
  'fn order_graphql_error(',
  'fn shipping_profile_graphql_error_policy(',
  'order GraphQL mapper',
);
const shippingPolicy = between(
  facadeSource,
  'fn shipping_profile_graphql_error_policy(',
  'fn shipping_profile_graphql_error(',
  'shipping-profile GraphQL policy',
);
const shippingMapper = between(
  facadeSource,
  'fn shipping_profile_graphql_error(',
  'pub(crate) async fn ensure_storefront_order_access(',
  'shipping-profile GraphQL mapper',
);
const orderAccess = between(
  facadeSource,
  'pub(crate) async fn ensure_storefront_order_access(',
  'pub(crate) async fn validate_product_shipping_profile_input(',
  'storefront order access helper',
);
const productValidation = between(
  facadeSource,
  'pub(crate) async fn validate_product_shipping_profile_input(',
  'pub(crate) async fn validate_shipping_option_profile_inputs(',
  'product shipping-profile helper',
);
const optionValidation = facadeSource.slice(
  facadeSource.indexOf('pub(crate) async fn validate_shipping_option_profile_inputs('),
);

for (const [value, label] of [
  ['#[path = "safe_helpers.rs"]\nmod cart_safe_helpers;', 'private cart facade routing'],
  ['#[path = "safe_order_helpers.rs"]\npub mod helpers;', 'public order facade routing'],
]) requireText(moduleSource, value, label);

for (const [value, label] of [
  [
    'const STOREFRONT_ORDER_GRAPHQL_OWNER: &str = "rustok_order.storefront_access";',
    'order owner constant',
  ],
  [
    'const SHIPPING_PROFILE_GRAPHQL_OWNER: &str = "rustok_commerce.shipping_profiles";',
    'shipping owner constant',
  ],
  [
    'const STOREFRONT_GRAPHQL_HELPER_BOUNDARY: &str = "commerce_storefront_graphql_helper";',
    'GraphQL helper boundary',
  ],
  ['type StorefrontGraphqlPolicy = (', 'GraphQL policy type'],
  ['struct StorefrontOrderGraphqlErrorContext {', 'typed order context'],
  ['actor_id: Uuid,', 'order actor context'],
  ['customer_id: Uuid,', 'order customer context'],
  ['order_id: Option<Uuid>,', 'order identity context'],
  ['order_return_id: Option<Uuid>,', 'order-return identity context'],
  ['order_change_id: Option<Uuid>,', 'order-change identity context'],
  ['struct ShippingProfileGraphqlErrorContext<\'a> {', 'typed shipping context'],
  ['requested_slug: Option<&\'a str>,', 'requested slug context'],
  ['requested_profile_count: Option<usize>,', 'requested profile count'],
  ['shipping_profile_id: Option<Uuid>,', 'shipping-profile identity context'],
  ['fn single(', 'single-profile context constructor'],
  ['fn batch(', 'batch-profile context constructor'],
]) requireText(facadeSource, value, label);

for (const [value, label] of [
  ['async_graphql::Error::new(message)', 'static GraphQL public message'],
  ['extensions.set("code", code)', 'GraphQL public code extension'],
  ['extensions.set("retryable", retryable)', 'GraphQL retryability extension'],
]) requireText(facadeSource, value, label);

for (const [value, label] of [
  ['OrderError::Validation(_)', 'order validation mapping'],
  ['OrderError::OrderNotFound(order_id)', 'order not-found mapping'],
  ['context.order_id = Some(*order_id);', 'typed order identity adoption'],
  ['OrderError::OrderReturnNotFound(order_return_id)', 'order-return mapping'],
  ['context.order_return_id = Some(*order_return_id);', 'typed return identity adoption'],
  ['OrderError::OrderChangeNotFound(order_change_id)', 'order-change mapping'],
  ['context.order_change_id = Some(*order_change_id);', 'typed change identity adoption'],
  ['OrderError::InvalidTransition { .. }', 'order transition mapping'],
  ['OrderError::Database(_)', 'order database mapping'],
  ['OrderError::Core(_)', 'order fallback mapping'],
  ['"Order request is invalid"', 'order validation message'],
  ['"ORDER_REQUEST_INVALID"', 'order validation code'],
  ['"Order resource was not found"', 'order not-found message'],
  ['"ORDER_RESOURCE_NOT_FOUND"', 'order not-found code'],
  ['"Order operation conflicts with the current state"', 'order conflict message'],
  ['"ORDER_STATE_CONFLICT"', 'order conflict code'],
  ['"Order service is temporarily unavailable"', 'order availability message'],
  ['"ORDER_TEMPORARILY_UNAVAILABLE"', 'order availability code'],
  ['"Order operation could not be completed safely"', 'order fallback message'],
  ['"ORDER_OPERATION_FAILED"', 'order fallback code'],
]) requireText(orderPolicy, value, label);

for (const [value, label] of [
  ['error = ?error', 'typed order cause log'],
  ['owner = STOREFRONT_ORDER_GRAPHQL_OWNER', 'order owner log'],
  ['tenant_id = %context.tenant_id', 'order tenant log'],
  ['actor_id = %context.actor_id', 'order actor log'],
  ['customer_id = %context.customer_id', 'order customer log'],
  ['order_id = ?context.order_id', 'order identity log'],
  ['order_return_id = ?context.order_return_id', 'return identity log'],
  ['order_change_id = ?context.order_change_id', 'change identity log'],
  ['operation = %context.operation', 'order operation log'],
  ['error_kind,', 'order error-kind log'],
  ['public_code = code', 'order public-code log'],
  ['retryable,', 'order retryability log'],
  ['boundary = STOREFRONT_GRAPHQL_HELPER_BOUNDARY', 'order boundary log'],
  ['public_graphql_error(message, code, retryable)', 'order public envelope'],
]) requireText(orderMapper, value, label);

for (const [value, label] of [
  ['CommerceError::Validation(_)', 'shipping validation mapping'],
  ['CommerceError::InvalidPrice(_)', 'shipping price mapping'],
  ['CommerceError::InvalidOptionCombination', 'shipping option mapping'],
  ['CommerceError::NoVariants', 'shipping no-variants mapping'],
  [
    'CommerceError::ShippingProfileNotFound(shipping_profile_id)',
    'shipping not-found mapping',
  ],
  [
    'context.shipping_profile_id = Some(*shipping_profile_id);',
    'typed shipping identity adoption',
  ],
  ['CommerceError::DuplicateShippingProfileSlug(_)', 'shipping conflict mapping'],
  ['CommerceError::Database(_)', 'shipping database mapping'],
  ['CommerceError::ProductNotFound(_)', 'shipping fail-closed product mapping'],
  ['CommerceError::VariantNotFound(_)', 'shipping fail-closed variant mapping'],
  ['CommerceError::DuplicateHandle { .. }', 'shipping duplicate handle fallback'],
  ['CommerceError::DuplicateSku(_)', 'shipping duplicate SKU fallback'],
  ['CommerceError::InsufficientInventory { .. }', 'shipping inventory fallback'],
  ['CommerceError::CannotDeletePublished', 'shipping state fallback'],
  ['CommerceError::Rich(_)', 'shipping rich fallback'],
  ['CommerceError::Core(_)', 'shipping core fallback'],
  ['"Shipping profile request is invalid"', 'shipping validation message'],
  ['"SHIPPING_PROFILE_REQUEST_INVALID"', 'shipping validation code'],
  ['"Shipping profile was not found"', 'shipping not-found message'],
  ['"SHIPPING_PROFILE_NOT_FOUND"', 'shipping not-found code'],
  ['"Shipping profile conflicts with the current state"', 'shipping conflict message'],
  ['"SHIPPING_PROFILE_STATE_CONFLICT"', 'shipping conflict code'],
  ['"Shipping profile service is temporarily unavailable"', 'shipping availability message'],
  ['"SHIPPING_PROFILE_TEMPORARILY_UNAVAILABLE"', 'shipping availability code'],
  [
    '"Shipping profile operation could not be completed safely"',
    'shipping fallback message',
  ],
  ['"SHIPPING_PROFILE_OPERATION_FAILED"', 'shipping fallback code'],
]) requireText(shippingPolicy, value, label);

for (const [value, label] of [
  ['error = ?error', 'typed shipping cause log'],
  ['owner = SHIPPING_PROFILE_GRAPHQL_OWNER', 'shipping owner log'],
  ['tenant_id = %context.tenant_id', 'shipping tenant log'],
  ['requested_slug = ?context.requested_slug', 'requested slug log'],
  ['requested_profile_count = ?context.requested_profile_count', 'profile count log'],
  ['shipping_profile_id = ?context.shipping_profile_id', 'shipping identity log'],
  ['operation = %context.operation', 'shipping operation log'],
  ['error_kind,', 'shipping error-kind log'],
  ['public_code = code', 'shipping public-code log'],
  ['retryable,', 'shipping retryability log'],
  ['boundary = STOREFRONT_GRAPHQL_HELPER_BOUNDARY', 'shipping boundary log'],
  ['public_graphql_error(message, code, retryable)', 'shipping public envelope'],
]) requireText(shippingMapper, value, label);

for (const [value, label] of [
  ['ctx.data::<AuthContext>()', 'authenticated actor lookup'],
  ['resolve_optional_storefront_customer_id(', 'safe customer lookup reuse'],
  ['Some(auth),', 'authenticated customer context'],
  ['OrderService::new(db.clone(), event_bus.clone())', 'order service construction'],
  ['.get_order(tenant_id, order_id)', 'order owner call'],
  ['StorefrontOrderGraphqlErrorContext::new(', 'typed order mapper context'],
  ['auth.user_id,', 'actor identity forwarding'],
  ['customer_id,', 'customer identity forwarding'],
  ['"ensure_storefront_order_access"', 'order operation'],
  ['order.customer_id != Some(customer_id)', 'ownership comparison'],
  ['GraphQLError>::permission_denied(', 'ownership denial contract'],
]) requireText(orderAccess, value, label);

for (const [value, label] of [
  ['shipping_profile_slug.and_then(normalize_shipping_profile_slug)', 'slug normalization'],
  ['ShippingProfileService::new(db.clone())', 'shipping service construction'],
  ['.ensure_shipping_profile_slug_exists(tenant_id, &slug)', 'single slug owner call'],
  ['ShippingProfileGraphqlErrorContext::single(', 'single shipping context'],
  ['&slug,', 'normalized slug context'],
  ['"validate_product_shipping_profile_input"', 'single validation operation'],
]) requireText(productValidation, value, label);

for (const [value, label] of [
  ['let Some(slugs) = allowed_shipping_profile_slugs else', 'absent batch no-op'],
  ['ShippingProfileService::new(db.clone())', 'batch service construction'],
  ['.ensure_shipping_profile_slugs_exist(tenant_id, slugs.iter())', 'batch owner call'],
  ['ShippingProfileGraphqlErrorContext::batch(', 'batch shipping context'],
  ['slugs.len(),', 'batch profile count'],
  ['"validate_shipping_option_profile_inputs"', 'batch validation operation'],
]) requireText(optionValidation, value, label);

for (const [ownerSource, value, label] of [
  [orderErrors, 'Validation(String)', 'owner order validation variant'],
  [orderErrors, 'OrderNotFound(Uuid)', 'owner order-not-found variant'],
  [orderErrors, 'OrderReturnNotFound(Uuid)', 'owner return-not-found variant'],
  [orderErrors, 'OrderChangeNotFound(Uuid)', 'owner change-not-found variant'],
  [orderErrors, 'InvalidTransition { from: String, to: String }', 'owner transition variant'],
  [orderErrors, 'Database(#[from] DbErr)', 'owner order database variant'],
  [orderErrors, 'Core(#[from] rustok_core::Error)', 'owner order core variant'],
  [commerceErrors, 'ShippingProfileNotFound(Uuid)', 'owner shipping not-found variant'],
  [commerceErrors, 'DuplicateShippingProfileSlug(String)', 'owner shipping conflict variant'],
  [commerceErrors, 'Database(#[from] sea_orm::DbErr)', 'owner commerce database variant'],
  [commerceErrors, 'Rich(#[source] Box<RichError>)', 'owner commerce rich variant'],
  [commerceErrors, 'Core(#[from] CoreError)', 'owner commerce core variant'],
]) requireText(ownerSource, value, label);

for (const value of [
  'async_graphql::Error::new(error.message)',
  'async_graphql::Error::new(error.to_string())',
  'async_graphql::Error::new(format!("{error}"))',
  'public_graphql_error(error',
  'error.message',
  'err.to_string()',
  'eprintln!(',
  'dbg!(',
  'ensure_storefront_order_access,',
  'validate_product_shipping_profile_input,',
  'validate_shipping_option_profile_inputs,',
]) forbidText(facadeSource, value, 'order and shipping safe helper facade');

const orderMapperCalls = facadeSource.match(/order_graphql_error\(/g) ?? [];
if (orderMapperCalls.length !== 2) {
  failures.push(`expected order mapper definition plus one call, found ${orderMapperCalls.length}`);
}
const shippingMapperCalls = facadeSource.match(/shipping_profile_graphql_error\(/g) ?? [];
if (shippingMapperCalls.length !== 3) {
  failures.push(`expected shipping mapper definition plus two calls, found ${shippingMapperCalls.length}`);
}
const publicEnvelopeCalls = facadeSource.match(/public_graphql_error\(/g) ?? [];
if (publicEnvelopeCalls.length !== 3) {
  failures.push(`expected public envelope definition plus two mapper calls, found ${publicEnvelopeCalls.length}`);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL order and shipping helper error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL order access and shipping-profile helpers retain typed context and stable public envelopes',
);
