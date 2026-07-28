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
const apiPorts = read('crates/rustok-api/src/ports.rs');
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

const legacyMapper = between(
  shipping,
  'fn map_admin_shipping_option_error(',
  'fn admin_shipping_option_read_port_context(',
  'admin shipping-option mutation mapper',
);
const readContextBuilder = between(
  shipping,
  'fn admin_shipping_option_read_port_context(',
  'fn map_admin_shipping_option_port_error(',
  'admin shipping-option read context builder',
);
const portMapper = between(
  shipping,
  'fn map_admin_shipping_option_port_error(',
  'async fn validate_shipping_option_profile_inputs(',
  'admin shipping-option port mapper',
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
  ['PortActor, PortContext, PortError, PortErrorKind, RequestContext', 'typed port imports'],
  ['use rustok_fulfillment::error::FulfillmentError;', 'typed mutation error import'],
  ['ListAllShippingOptionProjectionsRequest', 'admin list request'],
  ['ReadShippingOptionProjectionRequest', 'admin lookup request'],
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
  ['PortActor::user(auth.user_id.to_string())', 'authenticated actor'],
  ['request_context.locale.as_str()', 'request locale'],
  ['format!("commerce-admin-shipping-option:{operation}:{resource_id}")', 'resource correlation id'],
  ['with_deadline(std::time::Duration::from_secs(2))', 'read deadline'],
  ['request_context.channel_slug.as_deref()', 'channel propagation'],
]) requireText(readContextBuilder, value, label);

for (const [value, label] of [
  ['error: PortError,', 'owned typed port cause'],
  ['PortErrorKind::Validation', 'validation kind'],
  ['PortErrorKind::NotFound', 'not-found kind'],
  ['PortErrorKind::Conflict', 'conflict kind'],
  ['PortErrorKind::Forbidden', 'forbidden kind'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'unavailable kinds'],
  ['PortErrorKind::InvariantViolation', 'invariant kind'],
  ['StatusCode::BAD_REQUEST', 'bad-request status'],
  ['StatusCode::NOT_FOUND', 'not-found status'],
  ['StatusCode::CONFLICT', 'conflict status'],
  ['StatusCode::UNAUTHORIZED', 'forbidden status'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'unavailable status'],
  ['StatusCode::INTERNAL_SERVER_ERROR', 'invariant status'],
  ['"commerce_admin_fulfillment_invalid"', 'validation code'],
  ['"commerce_admin_not_found"', 'not-found code'],
  ['"commerce_admin_fulfillment_state_conflict"', 'conflict code'],
  ['"commerce_permission_denied"', 'forbidden code'],
  ['"commerce_admin_fulfillment_storage_unavailable"', 'storage code'],
  ['"commerce_admin_fulfillment_failed"', 'fail-closed code'],
  ['error = ?error', 'typed internal cause'],
  ['owner_operation,', 'owner operation log'],
  ['correlation_id = %port_context.correlation_id', 'correlation log'],
  ['actor = ?port_context.actor', 'actor log'],
  ['channel = ?port_context.channel', 'channel log'],
  ['locale = %port_context.locale', 'locale log'],
  ['deadline_ms = ?port_context.deadline_ms', 'deadline log'],
  ['internal_code = %error.code', 'owner code log'],
  ['retryable = error.retryable', 'retryability log'],
  ['HttpError::new(status, code, message)', 'static public envelope'],
]) requireText(portMapper, value, label);

for (const [block, operation, request, label] of [
  [
    listRoute,
    '.list_all_shipping_option_projections(',
    'ListAllShippingOptionProjectionsRequest {',
    'list route',
  ],
  [
    showRoute,
    '.read_shipping_option_projection(',
    'ReadShippingOptionProjectionRequest {',
    'show route',
  ],
]) {
  requireText(block, 'admin_shipping_option_read_port_context(', `${label} context construction`);
  requireText(block, operation, `${label} owner operation`);
  requireText(block, request, `${label} typed request`);
  requireText(block, 'map_admin_shipping_option_port_error(', `${label} typed mapper`);
  requireText(block, 'requested_locale: Some(request_context.locale.clone())', `${label} locale`);
  requireText(block, 'tenant_default_locale: Some(tenant.default_locale.clone())', `${label} fallback locale`);
  forbidText(block, 'FulfillmentService::new(', `${label} concrete service`);
}

for (const [block, operation, label] of [
  [createRoute, '.create_shipping_option(tenant.id, input)', 'create route'],
  [updateRoute, '.update_shipping_option(tenant.id, id, input)', 'update route'],
  [deactivateRoute, '.deactivate_shipping_option(tenant.id, id)', 'deactivate route'],
  [reactivateRoute, '.reactivate_shipping_option(tenant.id, id)', 'reactivate route'],
]) {
  requireText(block, 'FulfillmentService::new(runtime.db_clone())', `${label} lifecycle service`);
  requireText(block, operation, `${label} service operation`);
  requireText(block, 'map_admin_shipping_option_error(', `${label} legacy typed mapper`);
}

for (const [value, label] of [
  ['FulfillmentError::Validation(_)', 'mutation validation variant'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'mutation not-found variant'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'mutation fulfillment variant'],
  ['FulfillmentError::InvalidTransition { .. }', 'mutation conflict variant'],
  ['FulfillmentError::Database(_)', 'mutation database variant'],
]) requireText(legacyMapper, value, label);

for (const [content, value, label] of [
  [apiPorts, 'pub enum PortErrorKind {', 'owner port error kinds'],
  [apiPorts, 'InvariantViolation,', 'owner invariant kind'],
  [fulfillmentErrors, 'ShippingOptionNotFound(Uuid)', 'owner shipping-option variant'],
]) requireText(content, value, label);

for (const value of [
  'error.to_string()',
  'err.to_string()',
  'other.to_string()',
  'error.message',
  'format!("{}: {}", error.code, error.message)',
  '.map_err(super::map_fulfillment_error)?;',
]) forbidText(shipping, value, 'unsafe admin shipping-option public conversion');

const legacyMapperUses = shipping.match(/map_admin_shipping_option_error\(/g) ?? [];
if (legacyMapperUses.length !== 5) {
  failures.push(`expected mutation mapper definition plus four uses, found ${legacyMapperUses.length}`);
}
const portMapperUses = shipping.match(/map_admin_shipping_option_port_error\(/g) ?? [];
if (portMapperUses.length !== 3) {
  failures.push(`expected port mapper definition plus two read uses, found ${portMapperUses.length}`);
}

if (failures.length > 0) {
  console.error('Commerce admin shipping-option error-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin shipping-option reads use typed owner-port context while mutations retain typed lifecycle envelopes',
);
