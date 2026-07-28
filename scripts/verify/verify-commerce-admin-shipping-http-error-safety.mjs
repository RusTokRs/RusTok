#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const shipping = read('crates/rustok-commerce/src/controllers/admin/shipping.rs');
const commerceErrors = read('crates/rustok-commerce-foundation/src/error.rs');
const fulfillmentErrors = read('crates/rustok-fulfillment/src/error.rs');
const apiPorts = read('crates/rustok-api/src/ports.rs');
const shippingService = read('crates/rustok-commerce/src/services/shipping_profile.rs');
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

const profileMapper = between(
  shipping,
  'fn map_shipping_profile_error(',
  'fn map_admin_shipping_option_error(',
  'shipping-profile mapper',
);
const mutationMapper = between(
  shipping,
  'fn map_admin_shipping_option_error(',
  'fn admin_shipping_option_read_port_context(',
  'shipping-option mutation mapper',
);
const readContext = between(
  shipping,
  'fn admin_shipping_option_read_port_context(',
  'fn map_admin_shipping_option_port_error(',
  'shipping-option read context',
);
const readMapper = between(
  shipping,
  'fn map_admin_shipping_option_port_error(',
  'async fn validate_shipping_option_profile_inputs(',
  'shipping-option read mapper',
);
const listRoute = between(
  shipping,
  'pub async fn list_shipping_options(',
  '/// Create admin shipping option',
  'shipping-option list route',
);
const createRoute = between(
  shipping,
  'pub async fn create_shipping_option(',
  '/// Show admin shipping option',
  'shipping-option create route',
);
const showRoute = between(
  shipping,
  'pub async fn show_shipping_option(',
  '/// Update admin shipping option',
  'shipping-option show route',
);
const updateRoute = between(
  shipping,
  'pub async fn update_shipping_option(',
  '/// Deactivate admin shipping option',
  'shipping-option update route',
);
const deactivateRoute = between(
  shipping,
  'pub async fn deactivate_shipping_option(',
  '/// Reactivate admin shipping option',
  'shipping-option deactivate route',
);
const reactivateStart = shipping.indexOf('pub async fn reactivate_shipping_option(');
const reactivateRoute = reactivateStart < 0 ? '' : shipping.slice(reactivateStart);
if (reactivateStart < 0) failures.push('shipping-option reactivate route: unable to isolate source block');

for (const [value, label] of [
  ['CommerceError, ShippingProfileService', 'typed commerce error import'],
  ['use rustok_fulfillment::error::FulfillmentError;', 'typed mutation error import'],
  ['PortActor, PortContext, PortError, PortErrorKind, RequestContext', 'typed read error imports'],
  ['ListAllShippingOptionProjectionsRequest', 'typed admin list request'],
  ['ReadShippingOptionProjectionRequest', 'typed admin lookup request'],
  ['fn map_shipping_profile_error(error: CommerceError)', 'profile mapper'],
  ['fn map_admin_shipping_option_error(', 'mutation mapper'],
  ['fn admin_shipping_option_read_port_context(', 'read context builder'],
  ['fn map_admin_shipping_option_port_error(', 'read port mapper'],
  ['async fn validate_shipping_option_profile_inputs(', 'profile validation helper'],
  ['const ADMIN_SHIPPING_BOUNDARY: &str = "commerce_admin_shipping_http";', 'HTTP boundary'],
]) requireText(shipping, value, label);

for (const [value, label] of [
  ['CommerceError::ShippingProfileNotFound(_)', 'profile not-found mapping'],
  ['CommerceError::DuplicateShippingProfileSlug(_)', 'profile conflict mapping'],
  ['CommerceError::Validation(_)', 'profile validation mapping'],
  ['CommerceError::Database(_)', 'profile database mapping'],
  ['StatusCode::NOT_FOUND', 'profile not-found status'],
  ['StatusCode::CONFLICT', 'profile conflict status'],
  ['StatusCode::BAD_REQUEST', 'profile validation status'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'profile unavailable status'],
  ['StatusCode::INTERNAL_SERVER_ERROR', 'profile fail-closed status'],
  ['"commerce_admin_shipping_profile_conflict"', 'profile conflict code'],
  ['"commerce_admin_shipping_profile_invalid"', 'profile invalid code'],
  ['"commerce_admin_shipping_profile_storage_unavailable"', 'profile storage code'],
  ['"commerce_admin_shipping_profile_failed"', 'profile fail-closed code'],
  ['HttpError::new(status, code, message)', 'profile static envelope'],
]) requireText(profileMapper, value, label);

for (const [value, label] of [
  ['FulfillmentError::Validation(_)', 'mutation validation mapping'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'mutation not-found mapping'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'mutation fulfillment mapping'],
  ['FulfillmentError::InvalidTransition { .. }', 'mutation conflict mapping'],
  ['FulfillmentError::Database(_)', 'mutation unavailable mapping'],
  ['"commerce_admin_fulfillment_invalid"', 'mutation invalid code'],
  ['"commerce_admin_not_found"', 'mutation not-found code'],
  ['"commerce_admin_fulfillment_state_conflict"', 'mutation conflict code'],
  ['"commerce_admin_fulfillment_storage_unavailable"', 'mutation unavailable code'],
  ['HttpError::new(status, code, message)', 'mutation static envelope'],
]) requireText(mutationMapper, value, label);

for (const [value, label] of [
  ['PortActor::user(auth.user_id.to_string())', 'read user actor'],
  ['request_context.locale.as_str()', 'read locale'],
  ['format!("commerce-admin-shipping-option:{operation}:{resource_id}")', 'read correlation id'],
  ['with_deadline(std::time::Duration::from_secs(2))', 'read deadline'],
  ['request_context.channel_slug.as_deref()', 'read channel'],
]) requireText(readContext, value, label);

for (const [value, label] of [
  ['PortErrorKind::Validation', 'read validation kind'],
  ['PortErrorKind::NotFound', 'read not-found kind'],
  ['PortErrorKind::Conflict', 'read conflict kind'],
  ['PortErrorKind::Forbidden', 'read forbidden kind'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'read unavailable kinds'],
  ['PortErrorKind::InvariantViolation', 'read invariant kind'],
  ['StatusCode::BAD_REQUEST', 'read validation status'],
  ['StatusCode::NOT_FOUND', 'read not-found status'],
  ['StatusCode::CONFLICT', 'read conflict status'],
  ['StatusCode::UNAUTHORIZED', 'read forbidden status'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'read unavailable status'],
  ['StatusCode::INTERNAL_SERVER_ERROR', 'read invariant status'],
  ['"commerce_admin_fulfillment_invalid"', 'read validation code'],
  ['"commerce_admin_not_found"', 'read not-found code'],
  ['"commerce_admin_fulfillment_state_conflict"', 'read conflict code'],
  ['"commerce_permission_denied"', 'read forbidden code'],
  ['"commerce_admin_fulfillment_storage_unavailable"', 'read unavailable code'],
  ['"commerce_admin_fulfillment_failed"', 'read invariant code'],
  ['error = ?error', 'typed owner cause'],
  ['correlation_id = %port_context.correlation_id', 'correlation log'],
  ['internal_code = %error.code', 'owner code log'],
  ['retryable = error.retryable', 'retryability log'],
  ['HttpError::new(status, code, message)', 'read static envelope'],
]) requireText(readMapper, value, label);

for (const [block, getter, operation, request, label] of [
  [
    listRoute,
    '.shipping_option_admin_read_port()',
    '.list_all_shipping_option_projections(',
    'ListAllShippingOptionProjectionsRequest {',
    'list route',
  ],
  [
    showRoute,
    '.shipping_option_read_port()',
    '.read_shipping_option_projection(',
    'ReadShippingOptionProjectionRequest {',
    'show route',
  ],
]) {
  requireText(block, getter, `${label} host runtime getter`);
  requireText(block, operation, `${label} owner operation`);
  requireText(block, request, `${label} typed request`);
  requireText(block, 'map_admin_shipping_option_port_error(', `${label} port mapper`);
  requireText(block, 'requested_locale: Some(request_context.locale.clone())', `${label} locale forwarding`);
  requireText(block, 'tenant_default_locale: Some(tenant.default_locale.clone())', `${label} fallback forwarding`);
  forbidText(block, 'FulfillmentService::new(', `${label} concrete read service`);
}

for (const [block, operation, label] of [
  [createRoute, '.create_shipping_option(tenant.id, input)', 'create route'],
  [updateRoute, '.update_shipping_option(tenant.id, id, input)', 'update route'],
  [deactivateRoute, '.deactivate_shipping_option(tenant.id, id)', 'deactivate route'],
  [reactivateRoute, '.reactivate_shipping_option(tenant.id, id)', 'reactivate route'],
]) {
  requireText(block, 'FulfillmentService::new(runtime.db_clone())', `${label} concrete mutation service`);
  requireText(block, operation, `${label} mutation operation`);
  requireText(block, 'map_admin_shipping_option_error(', `${label} mutation mapper`);
}

for (const [value, label] of [
  ['items.retain(|option| option.active == active)', 'active filter'],
  ['option.currency_code.eq_ignore_ascii_case(currency_code)', 'currency filter'],
  ['option.provider_id.eq_ignore_ascii_case(provider_id)', 'provider filter'],
  ['option.name.to_ascii_lowercase().contains(&search)', 'search filter'],
  ['skip(pagination.offset() as usize)', 'pagination offset'],
  ['take(pagination.limit() as usize)', 'pagination limit'],
  ['validate_shipping_option_profile_inputs(', 'profile validation helper'],
]) requireText(shipping, value, label);

for (const [content, value, label] of [
  [commerceErrors, 'Database(#[from] sea_orm::DbErr)', 'commerce database variant'],
  [commerceErrors, 'Validation(String)', 'commerce validation variant'],
  [fulfillmentErrors, 'Validation(String)', 'fulfillment validation variant'],
  [fulfillmentErrors, 'ShippingOptionNotFound(Uuid)', 'fulfillment shipping-option variant'],
  [fulfillmentErrors, 'FulfillmentNotFound(Uuid)', 'fulfillment resource variant'],
  [fulfillmentErrors, 'InvalidTransition { from: String, to: String }', 'fulfillment transition variant'],
  [fulfillmentErrors, 'Database(#[from] DbErr)', 'fulfillment database variant'],
  [apiPorts, 'pub enum PortErrorKind {', 'port kind enum'],
  [apiPorts, 'InvariantViolation,', 'invariant port kind'],
  [shippingService, 'CommerceError::Validation(error.to_string())', 'profile validation construction'],
]) requireText(content, value, label);

for (const value of [
  'err.to_string()',
  'other.to_string()',
  'error.message',
  'format!("{}: {}", error.code, error.message)',
  'HttpError::bad_request("commerce_operation_failed"',
  '.map_err(super::map_fulfillment_error)?;',
]) forbidText(shipping, value, 'unsafe admin shipping public conversion');

const mutationMapperUses = shipping.match(/map_admin_shipping_option_error\(/g) ?? [];
if (mutationMapperUses.length !== 5) {
  failures.push(`expected mutation mapper definition plus four uses, found ${mutationMapperUses.length}`);
}
const readMapperUses = shipping.match(/map_admin_shipping_option_port_error\(/g) ?? [];
if (readMapperUses.length !== 3) {
  failures.push(`expected read mapper definition plus two uses, found ${readMapperUses.length}`);
}
const localValidationUses = shipping.match(/validate_shipping_option_profile_inputs\(/g) ?? [];
if (localValidationUses.length !== 3) {
  failures.push(`expected validation helper definition plus two uses, found ${localValidationUses.length}`);
}

if (failures.length > 0) {
  console.error('Commerce admin shipping HTTP error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin shipping reads use host-composed owner ports and stable HTTP envelopes while mutations retain lifecycle services',
);
