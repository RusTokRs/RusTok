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
const fulfillmentErrors = read('crates/rustok-fulfillment/src/error.rs');
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

const mapper = between(
  shipping,
  'fn map_admin_shipping_option_error(',
  'async fn validate_shipping_option_profile_inputs(',
  'admin shipping-option mapper',
);
const listRoute = between(
  shipping,
  'pub async fn list_shipping_options(',
  '/// Create admin shipping option',
  'list shipping options route',
);
const createRoute = between(
  shipping,
  'pub async fn create_shipping_option(',
  '/// Show admin shipping option',
  'create shipping option route',
);
const showRoute = between(
  shipping,
  'pub async fn show_shipping_option(',
  '/// Update admin shipping option',
  'show shipping option route',
);
const updateRoute = between(
  shipping,
  'pub async fn update_shipping_option(',
  '/// Deactivate admin shipping option',
  'update shipping option route',
);
const deactivateRoute = between(
  shipping,
  'pub async fn deactivate_shipping_option(',
  '/// Reactivate admin shipping option',
  'deactivate shipping option route',
);
const reactivateStart = shipping.indexOf('pub async fn reactivate_shipping_option(');
const reactivateRoute = reactivateStart < 0 ? '' : shipping.slice(reactivateStart);
if (reactivateStart < 0) failures.push('reactivate shipping option route: unable to isolate source block');

for (const [value, label] of [
  ['use rustok_fulfillment::error::FulfillmentError;', 'typed fulfillment error import'],
  [
    'const ADMIN_SHIPPING_OPTION_OWNER: &str = "rustok_fulfillment.admin_shipping_options";',
    'owner constant',
  ],
  [
    'const ADMIN_SHIPPING_BOUNDARY: &str = "commerce_admin_shipping_http";',
    'boundary constant',
  ],
  ['struct AdminShippingOptionErrorContext {', 'error context'],
  ['tenant_id: Uuid,', 'tenant field'],
  ['shipping_option_id: Option<Uuid>,', 'option identity field'],
  ["operation: &'static str,", 'operation field'],
]) requireText(shipping, value, label);

for (const [value, label] of [
  ['error: FulfillmentError,', 'owned typed cause'],
  ['FulfillmentError::Validation(_)', 'validation variant'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'shipping-option not-found variant'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'fulfillment not-found variant'],
  ['FulfillmentError::InvalidTransition { .. }', 'transition variant'],
  ['FulfillmentError::Database(_)', 'database variant'],
  ['StatusCode::BAD_REQUEST', 'bad-request status'],
  ['StatusCode::NOT_FOUND', 'not-found status'],
  ['StatusCode::CONFLICT', 'conflict status'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'unavailable status'],
  ['"commerce_admin_fulfillment_invalid"', 'validation code'],
  ['"commerce_admin_not_found"', 'not-found code'],
  ['"commerce_admin_fulfillment_state_conflict"', 'conflict code'],
  ['"commerce_admin_fulfillment_storage_unavailable"', 'storage code'],
  ['"Fulfillment request is invalid"', 'static validation message'],
  ['"Commerce resource not found"', 'static not-found message'],
  [
    '"Fulfillment operation conflicts with the current state"',
    'static conflict message',
  ],
  ['"Fulfillment storage is temporarily unavailable"', 'static storage message'],
  ['error = ?error', 'typed internal cause'],
  ['owner = ADMIN_SHIPPING_OPTION_OWNER', 'owner log'],
  ['tenant_id = %context.tenant_id', 'tenant log'],
  ['shipping_option_id = ?context.shipping_option_id', 'option identity log'],
  ['operation = %context.operation', 'operation log'],
  ['error_kind,', 'error-kind log'],
  ['public_code = code', 'public-code log'],
  ['status = %status', 'status log'],
  ['boundary = ADMIN_SHIPPING_BOUNDARY', 'boundary log'],
  ['HttpError::new(status, code, message)', 'single static envelope constructor'],
]) requireText(mapper, value, label);

for (const [block, operation, identity, serviceCall, label] of [
  [
    listRoute,
    '"list_shipping_options"',
    'tenant.id, None,',
    '.list_all_shipping_options(',
    'list route',
  ],
  [
    createRoute,
    '"create_shipping_option"',
    'tenant.id, None,',
    '.create_shipping_option(tenant.id, input)',
    'create route',
  ],
  [
    showRoute,
    '"get_shipping_option"',
    'tenant.id,\n                    Some(id),',
    '.get_shipping_option(',
    'show route',
  ],
  [
    updateRoute,
    '"update_shipping_option"',
    'tenant.id,\n                    Some(id),',
    '.update_shipping_option(tenant.id, id, input)',
    'update route',
  ],
  [
    deactivateRoute,
    '"deactivate_shipping_option"',
    'tenant.id,\n                    Some(id),',
    '.deactivate_shipping_option(tenant.id, id)',
    'deactivate route',
  ],
  [
    reactivateRoute,
    '"reactivate_shipping_option"',
    'tenant.id,\n                    Some(id),',
    '.reactivate_shipping_option(tenant.id, id)',
    'reactivate route',
  ],
]) {
  requireText(block, '.map_err(|error| {', `${label} typed mapping closure`);
  requireText(block, 'map_admin_shipping_option_error(', `${label} mapper handoff`);
  requireText(block, 'AdminShippingOptionErrorContext::new(', `${label} context construction`);
  requireText(block, operation, `${label} operation`);
  requireText(block, identity, `${label} truthful identity`);
  requireText(block, serviceCall, `${label} service contract`);
}

for (const [value, label] of [
  ['[Permission::FULFILLMENTS_READ]', 'read permission'],
  ['[Permission::FULFILLMENTS_CREATE]', 'create permission'],
  ['[Permission::FULFILLMENTS_UPDATE]', 'update permission'],
  ['page: pagination.page', 'pagination page forwarding'],
  ['per_page: pagination.limit()', 'pagination size forwarding'],
  ['items.retain(|option| option.active == active)', 'active filter'],
  ['option.currency_code.eq_ignore_ascii_case(currency_code)', 'currency filter'],
  ['option.provider_id.eq_ignore_ascii_case(provider_id)', 'provider filter'],
  ['option.name.to_ascii_lowercase().contains(&search)', 'search filter'],
  ['validate_shipping_option_profile_inputs(', 'profile validation helper'],
]) requireText(shipping, value, label);

const mapperUses =
  shipping.match(
    /map_admin_shipping_option_error\(\s+AdminShippingOptionErrorContext::new\(/g,
  ) ?? [];
if (mapperUses.length !== 6) {
  failures.push(`expected six context-aware shipping-option mapper callsites, found ${mapperUses.length}`);
}

for (const [value, label] of [
  ['Validation(String)', 'owner validation variant'],
  ['ShippingOptionNotFound(Uuid)', 'owner shipping-option variant'],
  ['FulfillmentNotFound(Uuid)', 'owner fulfillment variant'],
  ['InvalidTransition { from: String, to: String }', 'owner transition variant'],
  ['Database(#[from] DbErr)', 'owner database variant'],
]) requireText(fulfillmentErrors, value, label);

for (const value of [
  '.map_err(super::map_fulfillment_error)?;',
  'format!("Fulfillment request is invalid:',
  'error.to_string()',
  'err.to_string()',
  'other.to_string()',
  'HttpError::bad_request("commerce_operation_failed"',
]) forbidText(shipping, value, 'unsafe admin shipping-option public conversion');

if (failures.length > 0) {
  console.error('Commerce admin shipping-option error-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin shipping-option errors retain route context and static public envelopes',
);
