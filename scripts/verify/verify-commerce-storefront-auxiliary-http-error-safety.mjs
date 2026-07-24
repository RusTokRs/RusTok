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
const apiPorts = read('crates/rustok-api/src/ports.rs');
const webErrors = read('crates/rustok-web/src/lib.rs');
const commerceErrors = read('crates/rustok-commerce-foundation/src/error.rs');
const fulfillmentErrors = read('crates/rustok-fulfillment/src/error.rs');
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

const portMapper = between(
  controller,
  'fn map_storefront_auxiliary_port_error(',
  'fn storefront_auxiliary_public_error<E>(',
  'storefront auxiliary port mapper',
);
const shippingContextMapper = between(
  controller,
  'fn map_storefront_shipping_context_error(',
  'fn map_storefront_fulfillment_error(',
  'storefront shipping context mapper',
);
const fulfillmentMapper = between(
  controller,
  'fn map_storefront_fulfillment_error(',
  '/// List published storefront products',
  'storefront fulfillment mapper',
);
const regionHandler = between(
  controller,
  'pub async fn list_regions(',
  '/// List active storefront shipping options',
  'storefront region handler',
);
const shippingHandler = controller.slice(controller.indexOf('pub async fn list_shipping_options('));
const auxiliaryBoundary = controller.slice(controller.indexOf('/// List available storefront regions'));

for (const [value, label] of [
  ['PortError, RequestContext', 'typed port error import'],
  ['FulfillmentService, error::FulfillmentError', 'typed fulfillment error import'],
  ['port_error_to_http_error', 'shared port HTTP mapper import'],
  ['fn map_storefront_auxiliary_port_error(', 'auxiliary port mapper'],
  ['fn storefront_auxiliary_public_error<E>(', 'static auxiliary envelope helper'],
  ['fn map_storefront_shipping_context_error(', 'shipping context mapper'],
  ['fn map_storefront_fulfillment_error(', 'fulfillment mapper'],
  ['boundary = "commerce_storefront_auxiliary_http"', 'auxiliary boundary log'],
  ['error = ?error', 'raw owner error log'],
  ['owner,', 'owner log field'],
  ['operation,', 'operation log field'],
  ['tenant_id = %tenant_id', 'tenant log field'],
  ['cart_id = ?cart_id', 'cart log field'],
  ['public_code = code', 'public code log field'],
  ['status = %status', 'status log field'],
  ['HttpError::new(status, code, message)', 'static envelope construction'],
]) {
  requireText(controller, value, label);
}

for (const [value, label] of [
  ['port_error_to_http_error(error.clone())', 'shared port mapping'],
  ['error_kind = ?error.kind', 'typed port kind log'],
  ['retryable = error.retryable', 'port retryability log'],
  ['public_code = %public.code', 'mapped port code log'],
  ['status = %public.status', 'mapped port status log'],
]) {
  requireText(portMapper, value, label);
}

for (const [value, label] of [
  ['CommerceError::Database(_)', 'commerce database variant'],
  ['CommerceError::ProductNotFound(_)', 'commerce product not-found variant'],
  ['CommerceError::VariantNotFound(_)', 'commerce variant not-found variant'],
  ['CommerceError::ShippingProfileNotFound(_)', 'commerce profile not-found variant'],
  ['CommerceError::Validation(_)', 'commerce validation variant'],
  ['CommerceError::DuplicateHandle { .. }', 'commerce duplicate handle variant'],
  ['CommerceError::DuplicateSku(_)', 'commerce duplicate SKU variant'],
  ['CommerceError::InvalidPrice(_)', 'commerce invalid price variant'],
  ['CommerceError::InsufficientInventory { .. }', 'commerce inventory variant'],
  ['CommerceError::InvalidOptionCombination', 'commerce option variant'],
  ['CommerceError::DuplicateShippingProfileSlug(_)', 'commerce profile conflict variant'],
  ['CommerceError::NoVariants', 'commerce no-variants variant'],
  ['CommerceError::CannotDeletePublished', 'commerce state variant'],
  ['CommerceError::Rich(_)', 'commerce rich variant'],
  ['CommerceError::Core(_)', 'commerce core variant'],
  ['"commerce_store_shipping_invalid"', 'shipping invalid code'],
  ['"commerce_store_not_found"', 'storefront not-found code'],
  ['"commerce_store_shipping_unavailable"', 'shipping unavailable code'],
  ['"commerce_store_shipping_failed"', 'shipping fail-closed code'],
  ['StatusCode::BAD_REQUEST', 'shipping validation status'],
  ['StatusCode::NOT_FOUND', 'shipping not-found status'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'shipping unavailable status'],
  ['StatusCode::INTERNAL_SERVER_ERROR', 'shipping fail-closed status'],
]) {
  requireText(shippingContextMapper, value, label);
}

for (const [value, label] of [
  ['FulfillmentError::Validation(_)', 'fulfillment validation variant'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'shipping option not-found variant'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'fulfillment not-found variant'],
  ['FulfillmentError::InvalidTransition { .. }', 'fulfillment transition variant'],
  ['FulfillmentError::Database(_)', 'fulfillment database variant'],
  ['"commerce_store_shipping_invalid"', 'fulfillment invalid code'],
  ['"commerce_store_not_found"', 'fulfillment not-found code'],
  ['"commerce_store_shipping_state_conflict"', 'fulfillment state code'],
  ['"commerce_store_shipping_unavailable"', 'fulfillment unavailable code'],
  ['StatusCode::CONFLICT', 'fulfillment conflict status'],
]) {
  requireText(fulfillmentMapper, value, label);
}

for (const value of [
  'commerce_operation_failed',
  'err.to_string()',
  'error.to_string()',
  'error.message',
  'format!("{}: {}", error.code, error.message)',
  'HttpError::bad_request(',
]) {
  forbidText(auxiliaryBoundary, value, 'unsafe storefront auxiliary public conversion');
}

for (const [value, label] of [
  ['ensure_storefront_channel_enabled_for_db(', 'region channel guard'],
  ['RegionService::new(runtime.db_clone())', 'region service construction'],
  ['.list_regions_for_tenant(', 'region port call'],
  ['PortActor::service("commerce.store-regions")', 'region service actor'],
  ['format!("store-regions:{}", tenant.id)', 'region correlation identity'],
  ['with_deadline(std::time::Duration::from_secs(3))', 'region deadline'],
  ['requested_locale: Some(request_context.locale.clone())', 'region requested locale'],
  ['tenant_default_locale: Some(tenant.default_locale.clone())', 'region fallback locale'],
  ['"rustok_region"', 'region owner label'],
  ['"list_regions"', 'region operation label'],
  ['map_storefront_auxiliary_port_error(', 'region typed port mapper'],
  ['map(|projection| projection.region)', 'region projection response'],
]) {
  requireText(regionHandler, value, label);
}

for (const [value, label] of [
  ['ensure_storefront_channel_enabled_for_db(', 'shipping channel guard'],
  ['current_customer_id_for_db(', 'customer lookup'],
  ['let requested_cart_id = query.cart_id;', 'stable requested cart identity'],
  ['in_process_cart_storefront_port(runtime.db_clone())', 'cart port construction'],
  ['CartStorefrontReadRequest { cart_id }', 'cart read request'],
  ['"rustok_cart"', 'cart owner label'],
  ['"read_shipping_options_cart"', 'cart operation label'],
  ['ensure_store_cart_access(&cart, customer_id)', 'cart access guard'],
  ['load_cart_shipping_profile_slugs(runtime.db(), tenant.id, &cart)', 'shipping profile projection'],
  ['"load_cart_shipping_profiles"', 'profile operation label'],
  ['map_storefront_shipping_context_error(', 'typed profile mapper'],
  ['resolve_context_from_cart_for_db(', 'cart context resolution'],
  ['storefront_public_channel_slug_for_cart(&cart, &request_context)', 'cart channel resolution'],
  ['resolve_context_for_db(', 'query context resolution'],
  ['public_channel_slug_from_request(&request_context)', 'request channel resolution'],
  ['FulfillmentService::new(runtime.db_clone())', 'fulfillment service construction'],
  ['.list_shipping_options(', 'fulfillment option list'],
  ['Some(request_context.locale.as_str())', 'shipping requested locale'],
  ['Some(tenant.default_locale.as_str())', 'shipping fallback locale'],
  ['"list_shipping_options"', 'fulfillment operation label'],
  ['map_storefront_fulfillment_error(', 'typed fulfillment mapper'],
  ['option.currency_code.eq_ignore_ascii_case(currency_code)', 'currency filter'],
  ['is_metadata_visible_for_public_channel(', 'channel visibility filter'],
  ['is_shipping_option_compatible_with_profiles(', 'profile compatibility filter'],
  ['Ok(Json(options))', 'shipping response'],
]) {
  requireText(shippingHandler, value, label);
}

for (const [content, value, label] of [
  [apiPorts, 'pub struct PortError {', 'owner port error type'],
  [apiPorts, 'pub kind: PortErrorKind', 'owner port kind'],
  [apiPorts, 'pub code: String', 'owner port code'],
  [apiPorts, 'pub message: String', 'owner port message'],
  [apiPorts, 'pub retryable: bool', 'owner port retryability'],
  [webErrors, 'pub fn port_error_to_http_error(error: PortError)', 'shared port mapper'],
  [webErrors, 'PortErrorKind::Unavailable => StatusCode::SERVICE_UNAVAILABLE', 'port unavailable status'],
  [webErrors, 'PortErrorKind::Timeout => StatusCode::GATEWAY_TIMEOUT', 'port timeout status'],
  [webErrors, 'PortErrorKind::InvariantViolation => StatusCode::INTERNAL_SERVER_ERROR', 'port invariant status'],
  [webErrors, '"The requested service is temporarily unavailable"', 'safe unavailable message'],
  [webErrors, '"The requested operation could not be completed"', 'safe invariant message'],
  [commerceErrors, 'Database(#[from] sea_orm::DbErr)', 'owner commerce database variant'],
  [commerceErrors, 'Validation(String)', 'owner commerce validation variant'],
  [fulfillmentErrors, 'Validation(String)', 'owner fulfillment validation variant'],
  [fulfillmentErrors, 'ShippingOptionNotFound(Uuid)', 'owner shipping option variant'],
  [fulfillmentErrors, 'FulfillmentNotFound(Uuid)', 'owner fulfillment variant'],
  [fulfillmentErrors, 'InvalidTransition { from: String, to: String }', 'owner transition variant'],
  [fulfillmentErrors, 'Database(#[from] DbErr)', 'owner fulfillment database variant'],
  [fulfillmentService, '-> FulfillmentResult<Vec<ShippingOptionResponse>>', 'typed fulfillment list result'],
  [fulfillmentService, 'shipping_option::Column::TenantId.eq(tenant_id)', 'fulfillment tenant filter'],
  [fulfillmentService, 'shipping_option::Column::Active.eq(true)', 'active option filter'],
  [fulfillmentService, 'load_shipping_options_with_translations(', 'translation loading'],
  [storefrontShipping, 'pub async fn load_cart_shipping_profile_slugs(', 'profile projection function'],
  [storefrontShipping, '-> CommerceResult<BTreeSet<String>>', 'typed profile projection result'],
]) {
  requireText(content, value, label);
}

const portMapperUses = auxiliaryBoundary.match(/map_storefront_auxiliary_port_error\(/g) ?? [];
if (portMapperUses.length !== 2) {
  failures.push(`expected region and cart port mapper uses, found ${portMapperUses.length}`);
}
const shippingContextUses = auxiliaryBoundary.match(/map_storefront_shipping_context_error\(/g) ?? [];
if (shippingContextUses.length !== 1) {
  failures.push(`expected one shipping context mapper use, found ${shippingContextUses.length}`);
}
const fulfillmentMapperUses = auxiliaryBoundary.match(/map_storefront_fulfillment_error\(/g) ?? [];
if (fulfillmentMapperUses.length !== 1) {
  failures.push(`expected one fulfillment mapper use, found ${fulfillmentMapperUses.length}`);
}

if (failures.length > 0) {
  console.error('Commerce storefront auxiliary HTTP error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Storefront region and shipping-option reads use typed safe public envelopes');
