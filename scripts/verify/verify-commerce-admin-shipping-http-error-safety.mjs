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
const requireBefore = (content, first, second, label) => {
  const firstIndex = content.indexOf(first);
  const secondIndex = content.indexOf(second);
  if (firstIndex < 0 || secondIndex < 0 || firstIndex > secondIndex) {
    failures.push(`${label}: ${first} must precede ${second}`);
  }
};

const profileMapper = between(
  shipping,
  'fn map_shipping_profile_error(',
  'fn admin_shipping_option_command_idempotency_key',
  'shipping-profile mapper',
);
const ownerMapper = between(
  shipping,
  'fn map_admin_shipping_option_port_error(',
  'async fn validate_shipping_option_profile_inputs(',
  'shipping-option owner-port mapper',
);
const validationHelper = between(
  shipping,
  'async fn validate_shipping_option_profile_inputs(',
  '/// List admin shipping profiles',
  'shipping-profile validation helper',
);

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
  ['let error = AdminShippingDiagnosticError;', 'profile diagnostic shadow'],
  ['error = ?error', 'profile redacted error event'],
  ['owner = "rustok_commerce.shipping_profile"', 'profile owner event'],
  ['boundary = ADMIN_SHIPPING_BOUNDARY', 'profile boundary event'],
  ['HttpError::new(status, code, message)', 'profile static envelope'],
]) requireText(profileMapper, value, label);
requireBefore(
  profileMapper,
  'CommerceError::ShippingProfileNotFound(_)',
  'let error = AdminShippingDiagnosticError;',
  'profile typed policy selection',
);
requireBefore(
  profileMapper,
  'let error = AdminShippingDiagnosticError;',
  'tracing::error!(',
  'profile diagnostic shadow',
);

for (const [value, label] of [
  ['PortErrorKind::Validation', 'owner validation kind'],
  ['PortErrorKind::NotFound', 'owner not-found kind'],
  ['PortErrorKind::Conflict', 'owner conflict kind'],
  ['PortErrorKind::Forbidden', 'owner forbidden kind'],
  ['PortErrorKind::Unavailable | PortErrorKind::Timeout', 'owner unavailable kinds'],
  ['PortErrorKind::InvariantViolation', 'owner invariant kind'],
  ['StatusCode::UNAUTHORIZED', 'owner forbidden status'],
  ['StatusCode::INTERNAL_SERVER_ERROR', 'owner invariant status'],
  ['"commerce_permission_denied"', 'owner forbidden code'],
  ['"commerce_admin_fulfillment_failed"', 'owner invariant code'],
  ['let context = AdminShippingOptionDiagnosticContext::from(&context);', 'owner route context projection'],
  ['let port_context = AdminShippingOptionPortDiagnosticContext::from(port_context);', 'owner port context projection'],
  ['let error = AdminShippingOptionPortDiagnosticError {', 'owner diagnostic error shadow'],
  ['internal_code = %error.code', 'owner stable code'],
  ['retryable = error.retryable', 'owner retryability'],
  ['HttpError::new(status, code, message)', 'owner static envelope'],
]) requireText(ownerMapper, value, label);

for (const [value, label] of [
  ['ShippingProfileService::new(db.clone())', 'profile validation service'],
  ['.ensure_shipping_profile_slugs_exist(tenant_id, slugs.iter())', 'profile validation operation'],
  ['.map_err(map_shipping_profile_error)?;', 'profile validation mapper'],
]) requireText(validationHelper, value, label);

for (const route of [
  'list_shipping_profiles',
  'create_shipping_profile',
  'show_shipping_profile',
  'update_shipping_profile',
  'deactivate_shipping_profile',
  'reactivate_shipping_profile',
  'list_shipping_options',
  'create_shipping_option',
  'show_shipping_option',
  'update_shipping_option',
  'deactivate_shipping_option',
  'reactivate_shipping_option',
]) {
  requireText(shipping, `pub async fn ${route}(`, `${route} route`);
}

for (const [value, label] of [
  ['[Permission::FULFILLMENTS_READ]', 'read permission'],
  ['[Permission::FULFILLMENTS_CREATE]', 'create permission'],
  ['[Permission::FULFILLMENTS_UPDATE]', 'update permission'],
  ['.shipping_option_admin_read_port()', 'admin list read port'],
  ['.list_all_shipping_option_projections(', 'admin list read operation'],
  ['.shipping_option_read_port()', 'detail read port'],
  ['.read_shipping_option_projection(', 'detail read operation'],
  ['.shipping_option_admin_command_port()', 'shipping-option command owner'],
  ['.create_shipping_option(command_context.clone(), request)', 'create option operation'],
  ['.update_shipping_option(command_context.clone(), request)', 'update option operation'],
  ['.deactivate_shipping_option(command_context.clone(), request)', 'deactivate option operation'],
  ['.reactivate_shipping_option(command_context.clone(), request)', 'reactivate option operation'],
  ['admin_shipping_option_command_idempotency_key(', 'write admission identity'],
  ['admin_shipping_option_command_port_context(', 'write port context'],
  ['.with_idempotency_key(idempotency_key)', 'write idempotency context'],
  ['ShippingProfileService::new(runtime.db_clone())', 'shipping-profile owner'],
  ['.list_shipping_profiles(', 'list profile operation'],
  ['.create_shipping_profile(tenant.id, input)', 'create profile operation'],
  ['.get_shipping_profile(', 'show profile operation'],
  ['.update_shipping_profile(tenant.id, id, input)', 'update profile operation'],
  ['.deactivate_shipping_profile(tenant.id, id)', 'deactivate profile operation'],
  ['.reactivate_shipping_profile(tenant.id, id)', 'reactivate profile operation'],
  ['items.retain(|option| option.active == active)', 'active filter'],
  ['option.currency_code.eq_ignore_ascii_case(currency_code)', 'currency filter'],
  ['option.provider_id.eq_ignore_ascii_case(provider_id)', 'provider filter'],
  ['option.name.to_ascii_lowercase().contains(&search)', 'search filter'],
  ['skip(pagination.offset() as usize)', 'pagination offset'],
  ['take(pagination.limit() as usize)', 'pagination limit'],
  ['validate_shipping_option_profile_inputs(', 'profile validation helper use'],
]) requireText(shipping, value, label);

for (const value of [
  'error.to_string()',
  'err.to_string()',
  'other.to_string()',
  'error.message',
  'format!("{}: {}", error.code, error.message)',
  'HttpError::bad_request("commerce_operation_failed"',
  '.map_err(super::map_fulfillment_error)?;',
  'use rustok_fulfillment::error::FulfillmentError;',
  'FulfillmentService::new(runtime.db_clone())',
  'map_admin_shipping_option_error(',
]) forbidText(shipping, value, 'unsafe or legacy admin shipping public conversion');

const profileMapperUses = shipping.match(/map_shipping_profile_error/g) ?? [];
if (profileMapperUses.length !== 8) {
  failures.push(`expected profile mapper definition plus seven uses, found ${profileMapperUses.length}`);
}
const ownerMapperUses = shipping.match(/map_admin_shipping_option_port_error\(/g) ?? [];
if (ownerMapperUses.length !== 7) {
  failures.push(`expected owner mapper definition plus six uses, found ${ownerMapperUses.length}`);
}
const validationUses = shipping.match(/validate_shipping_option_profile_inputs\(/g) ?? [];
if (validationUses.length !== 3) {
  failures.push(`expected validation helper definition plus two uses, found ${validationUses.length}`);
}
const redactedDebugUses = shipping.match(/formatter\.write_str\("redacted"\)/g) ?? [];
if (redactedDebugUses.length !== 2) {
  failures.push(`expected two redacted diagnostic Debug implementations, found ${redactedDebugUses.length}`);
}
const traceUses = shipping.match(/tracing::error!\(/g) ?? [];
if (traceUses.length !== 3) {
  failures.push(`expected three bounded shipping error events, found ${traceUses.length}`);
}

if (failures.length > 0) {
  console.error('Commerce admin shipping HTTP error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin shipping profiles and options preserve HTTP policy through bounded owner ports',
);
