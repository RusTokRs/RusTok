#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const moduleSource = read('crates/modules/rustok-commerce/src/graphql/mutations/mod.rs');
const facadeSource = read('crates/modules/rustok-commerce/src/graphql/mutations/safe_order_helpers.rs');
const layeredSource = read(
  'crates/modules/rustok-commerce/src/graphql/mutations/layered_order_helpers.rs',
);
const commerceErrors = read('crates/modules/rustok-commerce-foundation/src/error.rs');
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
  'pub(crate) async fn validate_shipping_option_profile_inputs(',
  'storefront order access helper',
);
const optionValidation = facadeSource.slice(
  facadeSource.indexOf('pub(crate) async fn validate_shipping_option_profile_inputs('),
);

for (const [value, label] of [
  ['#[path = "safe_helpers.rs"]\nmod cart_safe_helpers;', 'private cart facade routing'],
  [
    '#[path = "safe_order_helpers.rs"]\nmod safe_order_helpers_impl;',
    'private order facade implementation routing',
  ],
  [
    '#[path = "layered_order_helpers.rs"]\npub mod helpers;',
    'public layered order facade routing',
  ],
  ['pub(crate) use super::safe_order_helpers_impl::*;', 'order helper parity re-export'],
]) requireText(`${moduleSource}\n${layeredSource}`, value, label);

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
  ['struct ShippingProfileGraphqlErrorContext {', 'typed shipping context'],
  ['requested_profile_count: Option<usize>,', 'requested profile count'],
  ['shipping_profile_id: Option<Uuid>,', 'shipping-profile identity context'],
  ['fn batch(', 'batch-profile context constructor'],
]) requireText(facadeSource, value, label);

for (const [value, label] of [
  ['async_graphql::Error::new(message)', 'static GraphQL public message'],
  ['extensions.set("code", code)', 'GraphQL public code extension'],
  ['extensions.set("retryable", retryable)', 'GraphQL retryability extension'],
]) requireText(facadeSource, value, label);

for (const value of ['Validation','NotFound','Conflict','Forbidden','Unavailable','Timeout','InvariantViolation']) {
  requireText(orderPolicy, `PortErrorKind::${value}`, 'Order owner port error policy');
}
requireText(orderPolicy, '"FORBIDDEN"', 'authorization classification');
for (const [value, label] of [
  ['owner_error_kind = ?error.kind', 'bounded typed order cause log'],
  ['owner = STOREFRONT_ORDER_GRAPHQL_OWNER', 'order owner log'],
  ['tenant_id_non_nil = !context.tenant_id.is_nil()', 'order tenant log'],
  ['actor_id_non_nil = !context.actor_id.is_nil()', 'order actor log'],
  ['customer_id_non_nil = !context.customer_id.is_nil()', 'order customer log'],
  ['order_id_non_nil = context.order_id.is_some_and', 'order identity log'],
  ['order_return_id_present = context.order_return_id.is_some()', 'return identity log'],
  ['order_change_id_present = context.order_change_id.is_some()', 'change identity log'],
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
  ['owner = SHIPPING_PROFILE_GRAPHQL_OWNER', 'shipping owner log'],
  ['tenant_id_non_nil = !context.tenant_id.is_nil()', 'shipping tenant log'],
  ['requested_profile_count = ?context.requested_profile_count', 'profile count log'],
  ['shipping_profile_id_present = context.shipping_profile_id.is_some()', 'shipping identity log'],
  ['operation = %context.operation', 'shipping operation log'],
  ['error_kind,', 'shipping error-kind log'],
  ['public_code = code', 'shipping public-code log'],
  ['retryable,', 'shipping retryability log'],
  ['boundary = STOREFRONT_GRAPHQL_HELPER_BOUNDARY', 'shipping boundary log'],
  ['public_graphql_error(message, code, retryable)', 'shipping public envelope'],
]) requireText(shippingMapper, value, label);

for (const [value, label] of [
  ['.data::<AuthContext>()', 'authenticated actor lookup'],
  ['resolve_optional_storefront_customer_id(', 'safe customer lookup reuse'],
  ['Some(auth),', 'authenticated customer context'],
  ['order_read_runtime_for_current_graphql_scope(', 'selected Order owner runtime'],
  ['.read_order_projection(', 'Order owner read port'],
  ['StorefrontOrderGraphqlErrorContext::new(', 'typed order mapper context'],
  ['auth.user_id,', 'actor identity forwarding'],
  ['customer_id,', 'customer identity forwarding'],
  ['"ensure_storefront_order_access"', 'order operation'],
  ['order.customer_id != Some(customer_id)', 'ownership comparison'],
  ['GraphQLError>::permission_denied(', 'ownership denial contract'],
]) requireText(orderAccess, value, label);

for (const [value, label] of [
  ['let Some(slugs) = allowed_shipping_profile_slugs else', 'absent batch no-op'],
  ['ShippingProfileService::new(db.clone())', 'batch service construction'],
  ['.ensure_shipping_profile_slugs_exist(tenant_id, slugs.iter())', 'batch owner call'],
  ['ShippingProfileGraphqlErrorContext::batch(', 'batch shipping context'],
  ['slugs.len(),', 'batch profile count'],
  ['"validate_shipping_option_profile_inputs"', 'batch validation operation'],
]) requireText(optionValidation, value, label);

for (const [ownerSource, value, label] of [
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
if (shippingMapperCalls.length !== 2) {
  failures.push(`expected shipping mapper definition plus one shipping-option call, found ${shippingMapperCalls.length}`);
}
const publicEnvelopeCalls = facadeSource.match(/public_graphql_error\(/g) ?? [];
if (publicEnvelopeCalls.length !== 5) {
  failures.push(`expected public envelope definition, two mappers, nil-order and missing-tenant refusals, found ${publicEnvelopeCalls.length}`);
}

for (const [block,label] of [[orderMapper,'Order'], [shippingMapper,'Shipping']]) {
  for(const marker of ['error = ?error', 'error.to_string()', 'error.message', 'detail.clone()',
    'tenant_id = %context.tenant_id', 'requested_slug =', 'shipping_profile_id = ?context.shipping_profile_id']) {
    forbidText(block, marker, `${label}: raw diagnostic payload`);
  }
}
for(const marker of ['detail.clone()', 'CommerceError::Validation(detail)', '(String,', '.to_string()']) {
  forbidText(shippingPolicy, marker, 'static shipping public policy');
}
requireText(shippingPolicy, ') -> StorefrontGraphqlPolicy', 'static shipping policy return type');
const readContext = between(facadeSource, 'fn storefront_order_read_context(',
  'fn shipping_profile_graphql_error_policy(', 'Order caller provenance');
for(const marker of ['request.correlation_id.clone()', 'Uuid::new_v4()',
  'request.locale.as_str()', 'request.channel_slug.as_deref()', 'with_claim(permission.to_string())']) {
  requireText(readContext, marker, 'trusted request provenance');
}
forbidText(readContext, 'format!(', 'resource-free correlation');
for(const marker of ['tenant.id != tenant_id', 'auth.tenant_id != tenant_id',
  'request.tenant_id != tenant_id', 'request.user_id != Some(auth.user_id)']) {
  requireText(orderAccess, marker, 'authenticated Order scope');
  if(orderAccess.indexOf(marker) > orderAccess.indexOf('resolve_optional_storefront_customer_id(')) {
    failures.push('Order authenticated scope must precede customer IO');
  }
}
requireText(orderAccess, '"Order context is temporarily unavailable"', 'missing trusted context sanitization');
forbidText(orderAccess, 'ctx.data::<TenantContext>()?;', 'raw framework extraction error');
if (failures.length > 0) {
  console.error('Commerce GraphQL order and shipping helper error safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL order access and shipping-profile helpers retain typed context, stable public envelopes, and layered routing',
);
