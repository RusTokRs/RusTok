#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const controller = read('crates/rustok-commerce/src/controllers/store/products.rs');
const httpRuntime = read('crates/rustok-commerce/src/controllers/mod.rs');
const apiPorts = read('crates/rustok-api/src/ports.rs');
const webErrors = read('crates/rustok-web/src/lib.rs');
const commerceErrors = read('crates/rustok-commerce-foundation/src/error.rs');
const shippingOwnerPort = read('crates/rustok-fulfillment/src/shipping_option_read.rs');
const fulfillmentService = read('crates/rustok-fulfillment/src/services/fulfillment.rs');
const storefrontShipping = read('crates/rustok-commerce/src/storefront_shipping.rs');
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const between = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = content.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return content.slice(startIndex, endIndex);
};

const auxiliaryPortMapper = between(
  controller,
  'fn map_storefront_auxiliary_port_error(',
  'fn storefront_auxiliary_public_error<E>(',
  'storefront auxiliary port mapper',
);
const shippingContextMapper = between(
  controller,
  'fn map_storefront_shipping_context_error(',
  'fn storefront_shipping_option_port_context(',
  'storefront shipping context mapper',
);
const shippingReadContext = between(
  controller,
  'fn storefront_shipping_option_port_context(',
  'fn map_storefront_shipping_port_error(',
  'storefront shipping read context',
);
const shippingPortMapper = between(
  controller,
  'fn map_storefront_shipping_port_error(',
  '/// List published storefront products',
  'storefront shipping port mapper',
);
const regionHandler = between(
  controller,
  'pub async fn list_regions(',
  '/// List active storefront shipping options',
  'storefront region handler',
);
const shippingStart = controller.indexOf('pub async fn list_shipping_options(');
const shippingHandler = shippingStart < 0 ? '' : controller.slice(shippingStart);
if (shippingStart < 0) failures.push('storefront shipping handler: unable to isolate source block');
const auxiliaryBoundary = controller.slice(controller.indexOf('/// List available storefront regions'));

for (const [value, label] of [
  ['OptionalAuthContext, PortActor, PortContext, PortError, PortErrorKind, RequestContext', 'typed port imports'],
  ['use rustok_fulfillment::ListShippingOptionProjectionsRequest;', 'typed shipping request'],
  ['fn map_storefront_auxiliary_port_error(', 'auxiliary port mapper'],
  ['fn map_storefront_shipping_context_error(', 'shipping context mapper'],
  ['fn storefront_shipping_option_port_context(', 'shipping read context'],
  ['fn map_storefront_shipping_port_error(', 'shipping port mapper'],
  ['boundary = "commerce_storefront_auxiliary_http"', 'HTTP boundary'],
]) requireText(controller, value, label);

for (const [value, label] of [
  ['port_error_to_http_error(error.clone())', 'shared auxiliary mapping'],
  ['error_kind = ?error.kind', 'auxiliary error kind'],
  ['retryable = error.retryable', 'auxiliary retryability'],
  ['public_code = %public.code', 'auxiliary public code'],
  ['status = %public.status', 'auxiliary public status'],
]) requireText(auxiliaryPortMapper, value, label);

for (const [value, label] of [
  ['CommerceError::Database(_)', 'context database variant'],
  ['CommerceError::ShippingProfileNotFound(_)', 'context profile variant'],
  ['CommerceError::Validation(_)', 'context validation variant'],
  ['"commerce_store_shipping_invalid"', 'context invalid code'],
  ['"commerce_store_not_found"', 'context not-found code'],
  ['"commerce_store_shipping_unavailable"', 'context unavailable code'],
  ['"commerce_store_shipping_failed"', 'context fail-closed code'],
  ['HttpError::new(status, code, message)', 'context static envelope'],
]) requireText(shippingContextMapper, value, label);

for (const [value, label] of [
  ['PortActor::user(value.user_id.to_string())', 'authenticated actor'],
  ['PortActor::service("rustok-commerce.storefront-shipping-options")', 'anonymous actor'],
  ['request_context.locale.as_str()', 'request locale'],
  ['format!("commerce-store-shipping-options:list:{resource_id}")', 'resource correlation id'],
  ['with_deadline(std::time::Duration::from_secs(2))', 'read deadline'],
  ['public_channel_slug: Option<&str>', 'effective channel input'],
  ['Some(channel) => context.with_channel(channel)', 'effective channel propagation'],
]) requireText(shippingReadContext, value, label);

for (const [value, label] of [
  ['PortErrorKind::Validation', 'validation kind'],
  ['PortErrorKind::NotFound', 'not-found kind'],
  ['PortErrorKind::Conflict', 'conflict kind'],
  ['PortErrorKind::Forbidden', 'forbidden kind'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'unavailable kinds'],
  ['PortErrorKind::InvariantViolation', 'invariant kind'],
  ['StatusCode::BAD_REQUEST', 'validation status'],
  ['StatusCode::NOT_FOUND', 'not-found status'],
  ['StatusCode::CONFLICT', 'conflict status'],
  ['StatusCode::UNAUTHORIZED', 'forbidden status'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'unavailable status'],
  ['StatusCode::INTERNAL_SERVER_ERROR', 'invariant status'],
  ['"commerce_store_shipping_invalid"', 'invalid code'],
  ['"commerce_store_not_found"', 'not-found code'],
  ['"commerce_store_shipping_state_conflict"', 'conflict code'],
  ['"commerce_store_denied"', 'forbidden code'],
  ['"commerce_store_shipping_unavailable"', 'unavailable code'],
  ['"commerce_store_shipping_failed"', 'invariant code'],
  ['owner_operation = "list_shipping_option_projections"', 'owner operation log'],
  ['correlation_id = %context.correlation_id', 'correlation log'],
  ['channel = ?context.channel', 'channel log'],
  ['deadline_ms = ?context.deadline_ms', 'deadline log'],
  ['internal_code = %error.code', 'owner code log'],
  ['retryable = error.retryable', 'retryability log'],
  ['HttpError::new(status, code, message)', 'static HTTP envelope'],
]) requireText(shippingPortMapper, value, label);

for (const [value, label] of [
  ['ensure_storefront_channel_enabled_for_db(', 'region channel guard'],
  ['.list_regions_for_tenant(', 'region port operation'],
  ['PortActor::service("commerce.store-regions")', 'region actor'],
  ['with_deadline(std::time::Duration::from_secs(3))', 'region deadline'],
  ['map_storefront_auxiliary_port_error(', 'region mapper'],
]) requireText(regionHandler, value, label);

for (const [value, label] of [
  ['ensure_storefront_channel_enabled_for_db(', 'shipping channel guard'],
  ['current_customer_id_for_db(', 'customer lookup'],
  ['let requested_cart_id = query.cart_id;', 'cart identity'],
  ['in_process_cart_storefront_port(runtime.db_clone())', 'cart port'],
  ['CartStorefrontReadRequest { cart_id }', 'cart request'],
  ['ensure_store_cart_access(&cart, customer_id)', 'cart access'],
  ['load_cart_shipping_profile_slugs(runtime.db(), tenant.id, &cart)', 'profile projection'],
  ['storefront_public_channel_slug_for_cart(&cart, &request_context)', 'cart effective channel'],
  ['public_channel_slug_from_request(&request_context)', 'request effective channel'],
  ['storefront_shipping_option_port_context(', 'owner context'],
  ['.shipping_option_read_port()', 'host-composed port'],
  ['.list_shipping_option_projections(', 'owner list operation'],
  ['ListShippingOptionProjectionsRequest {', 'typed owner request'],
  ['requested_locale: Some(request_context.locale.clone())', 'requested locale'],
  ['tenant_default_locale: Some(tenant.default_locale.clone())', 'default locale'],
  ['map_storefront_shipping_port_error(', 'typed owner mapper'],
  ['option.currency_code.eq_ignore_ascii_case(currency_code)', 'currency filter'],
  ['is_metadata_visible_for_public_channel(', 'channel filter'],
  ['is_shipping_option_compatible_with_profiles(', 'profile filter'],
  ['Ok(Json(options))', 'response'],
]) requireText(shippingHandler, value, label);

for (const value of ['FulfillmentService::new(', '.list_shipping_options(', 'map_storefront_fulfillment_error(']) {
  forbidText(shippingHandler, value, 'storefront shipping concrete read path');
}

for (const [content, value, label] of [
  [httpRuntime, 'shipping_option_read_runtime: crate::graphql_runtime::CommerceShippingOptionReadRuntime', 'HTTP runtime field'],
  [httpRuntime, 'fn shipping_option_read_port(', 'HTTP runtime getter'],
  [apiPorts, 'pub struct PortError {', 'port error type'],
  [apiPorts, 'pub enum PortErrorKind {', 'port error kinds'],
  [webErrors, 'pub fn port_error_to_http_error(error: PortError)', 'shared port HTTP mapper'],
  [commerceErrors, 'Validation(String)', 'commerce validation variant'],
  [shippingOwnerPort, 'pub trait ShippingOptionReadPort: Send + Sync', 'owner read port'],
  [shippingOwnerPort, 'context.require_policy(PortCallPolicy::read())?', 'owner read policy'],
  [shippingOwnerPort, '.list_shipping_options(', 'owner adapter active-list delegation'],
  [fulfillmentService, 'shipping_option::Column::Active.eq(true)', 'owner service active-only filter'],
  [storefrontShipping, 'pub async fn load_cart_shipping_profile_slugs(', 'profile projection function'],
]) requireText(content, value, label);

for (const value of [
  'commerce_operation_failed',
  'err.to_string()',
  'error.to_string()',
  'error.message',
  'format!("{}: {}", error.code, error.message)',
  'HttpError::bad_request(',
]) forbidText(auxiliaryBoundary, value, 'unsafe storefront auxiliary public conversion');

const auxiliaryMapperUses = auxiliaryBoundary.match(/map_storefront_auxiliary_port_error\(/g) ?? [];
if (auxiliaryMapperUses.length !== 2) {
  failures.push(`expected region and cart auxiliary mapper uses, found ${auxiliaryMapperUses.length}`);
}
const shippingContextUses = auxiliaryBoundary.match(/map_storefront_shipping_context_error\(/g) ?? [];
if (shippingContextUses.length !== 1) {
  failures.push(`expected one shipping context mapper use, found ${shippingContextUses.length}`);
}
const shippingPortUses = auxiliaryBoundary.match(/map_storefront_shipping_port_error\(/g) ?? [];
if (shippingPortUses.length !== 1) {
  failures.push(`expected one shipping owner-port mapper use, found ${shippingPortUses.length}`);
}

if (failures.length > 0) {
  console.error('Commerce storefront auxiliary HTTP error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Storefront region, cart, and shipping-option reads retain typed safe HTTP envelopes with host-composed shipping owner ports',
);
