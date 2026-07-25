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
const optionMapper = between(
  shipping,
  'fn map_admin_shipping_option_error(',
  'async fn validate_shipping_option_profile_inputs(',
  'shipping-option mapper',
);

for (const [value, label] of [
  ['CommerceError, ShippingProfileService', 'typed commerce error import'],
  ['use rustok_fulfillment::error::FulfillmentError;', 'typed fulfillment error import'],
  ['fn map_shipping_profile_error(error: CommerceError)', 'local shipping profile mapper'],
  ['fn map_admin_shipping_option_error(', 'local shipping option mapper'],
  ['async fn validate_shipping_option_profile_inputs(', 'local profile validation helper'],
  [
    'const ADMIN_SHIPPING_OPTION_OWNER: &str = "rustok_fulfillment.admin_shipping_options";',
    'shipping-option owner constant',
  ],
  [
    'const ADMIN_SHIPPING_BOUNDARY: &str = "commerce_admin_shipping_http";',
    'shipping HTTP boundary constant',
  ],
  ['struct AdminShippingOptionErrorContext {', 'shipping-option context'],
  ['tenant_id: Uuid,', 'tenant context field'],
  ['shipping_option_id: Option<Uuid>,', 'truthful option identity field'],
  ["operation: &'static str,", 'operation field'],
]) requireText(shipping, value, label);

for (const [value, label] of [
  ['CommerceError::ShippingProfileNotFound(_)', 'profile not-found mapping'],
  ['CommerceError::DuplicateShippingProfileSlug(_)', 'duplicate profile slug mapping'],
  ['CommerceError::Validation(_)', 'profile validation mapping'],
  ['CommerceError::Database(_)', 'profile database mapping'],
  ['StatusCode::NOT_FOUND', 'profile not-found status'],
  ['StatusCode::CONFLICT', 'profile conflict status'],
  ['StatusCode::BAD_REQUEST', 'profile bad-request status'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'profile unavailable status'],
  ['StatusCode::INTERNAL_SERVER_ERROR', 'profile fail-closed status'],
  ['"commerce_admin_not_found"', 'shared not-found code'],
  ['"commerce_admin_shipping_profile_conflict"', 'profile conflict code'],
  ['"commerce_admin_shipping_profile_invalid"', 'profile invalid code'],
  ['"commerce_admin_shipping_profile_storage_unavailable"', 'profile storage code'],
  ['"commerce_admin_shipping_profile_failed"', 'profile fail-closed code'],
  ['"unexpected_commerce_error"', 'unexpected owner variant kind'],
  ['error = ?error', 'profile typed internal cause'],
  ['owner = "rustok_commerce.shipping_profile"', 'profile owner logging'],
  ['boundary = ADMIN_SHIPPING_BOUNDARY', 'profile boundary logging'],
  ['HttpError::new(status, code, message)', 'profile static envelope construction'],
]) requireText(profileMapper, value, label);

for (const value of [
  'CommerceError::ProductNotFound(_)',
  'CommerceError::VariantNotFound(_)',
  'CommerceError::DuplicateHandle { .. }',
  'CommerceError::DuplicateSku(_)',
  'CommerceError::InvalidPrice(_)',
  'CommerceError::InsufficientInventory { .. }',
  'CommerceError::InvalidOptionCombination',
  'CommerceError::NoVariants',
  'CommerceError::CannotDeletePublished',
  'CommerceError::Rich(_)',
  'CommerceError::Core(_)',
]) requireText(profileMapper, value, 'fail-closed unrelated commerce variant');

for (const [value, label] of [
  ['error: FulfillmentError,', 'owned fulfillment cause'],
  ['FulfillmentError::Validation(_)', 'option validation mapping'],
  ['FulfillmentError::ShippingOptionNotFound(_)', 'option not-found mapping'],
  ['FulfillmentError::FulfillmentNotFound(_)', 'unexpected fulfillment not-found mapping'],
  ['FulfillmentError::InvalidTransition { .. }', 'option transition mapping'],
  ['FulfillmentError::Database(_)', 'option database mapping'],
  ['StatusCode::BAD_REQUEST', 'option bad-request status'],
  ['StatusCode::NOT_FOUND', 'option not-found status'],
  ['StatusCode::CONFLICT', 'option conflict status'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'option unavailable status'],
  ['"commerce_admin_fulfillment_invalid"', 'option validation code'],
  ['"commerce_admin_not_found"', 'option not-found code'],
  ['"commerce_admin_fulfillment_state_conflict"', 'option conflict code'],
  ['"commerce_admin_fulfillment_storage_unavailable"', 'option storage code'],
  ['"Fulfillment request is invalid"', 'static option validation message'],
  ['"Commerce resource not found"', 'static option not-found message'],
  [
    '"Fulfillment operation conflicts with the current state"',
    'static option conflict message',
  ],
  ['"Fulfillment storage is temporarily unavailable"', 'static option storage message'],
  ['error = ?error', 'option typed internal cause'],
  ['owner = ADMIN_SHIPPING_OPTION_OWNER', 'option owner logging'],
  ['tenant_id = %context.tenant_id', 'option tenant logging'],
  ['shipping_option_id = ?context.shipping_option_id', 'option identity logging'],
  ['operation = %context.operation', 'option operation logging'],
  ['error_kind,', 'option error-kind logging'],
  ['public_code = code', 'option public-code logging'],
  ['status = %status', 'option status logging'],
  ['boundary = ADMIN_SHIPPING_BOUNDARY', 'option boundary logging'],
  ['HttpError::new(status, code, message)', 'option static envelope construction'],
]) requireText(optionMapper, value, label);

for (const [value, label] of [
  ['Database(#[from] sea_orm::DbErr)', 'owner database variant'],
  ['ProductNotFound(Uuid)', 'owner product variant'],
  ['VariantNotFound(Uuid)', 'owner variant variant'],
  ['DuplicateHandle { handle: String, locale: String }', 'owner duplicate handle variant'],
  ['DuplicateSku(String)', 'owner duplicate SKU variant'],
  ['InvalidPrice(String)', 'owner invalid price variant'],
  ['InsufficientInventory { requested: i32, available: i32 }', 'owner inventory variant'],
  ['InvalidOptionCombination', 'owner option-combination variant'],
  ['Validation(String)', 'owner validation variant'],
  ['ShippingProfileNotFound(Uuid)', 'owner shipping profile variant'],
  ['DuplicateShippingProfileSlug(String)', 'owner duplicate profile slug variant'],
  ['NoVariants', 'owner no-variants variant'],
  ['CannotDeletePublished', 'owner published-delete variant'],
  ['Rich(#[source] Box<RichError>)', 'owner rich variant'],
  ['Core(#[from] CoreError)', 'owner core variant'],
]) requireText(commerceErrors, value, label);

for (const [value, label] of [
  ['Validation(String)', 'fulfillment validation variant'],
  ['ShippingOptionNotFound(Uuid)', 'fulfillment shipping option variant'],
  ['FulfillmentNotFound(Uuid)', 'fulfillment resource variant'],
  ['InvalidTransition { from: String, to: String }', 'fulfillment transition variant'],
  ['Database(#[from] DbErr)', 'fulfillment database variant'],
]) requireText(fulfillmentErrors, value, label);

for (const [value, label] of [
  ['CommerceError::Validation(error.to_string())', 'shipping service validation construction'],
  [
    'CommerceError::ShippingProfileNotFound(shipping_profile_id)',
    'shipping service not-found construction',
  ],
  ['CommerceError::DuplicateShippingProfileSlug(', 'shipping service conflict construction'],
  ['active_profile.insert(&self.db).await?;', 'shipping service database propagation'],
]) requireText(shippingService, value, label);

for (const [value, label] of [
  ['pub async fn list_shipping_profiles(', 'profile list handler'],
  ['pub async fn create_shipping_profile(', 'profile create handler'],
  ['pub async fn show_shipping_profile(', 'profile detail handler'],
  ['pub async fn update_shipping_profile(', 'profile update handler'],
  ['pub async fn deactivate_shipping_profile(', 'profile deactivate handler'],
  ['pub async fn reactivate_shipping_profile(', 'profile reactivate handler'],
  ['pub async fn list_shipping_options(', 'option list handler'],
  ['pub async fn create_shipping_option(', 'option create handler'],
  ['pub async fn show_shipping_option(', 'option detail handler'],
  ['pub async fn update_shipping_option(', 'option update handler'],
  ['pub async fn deactivate_shipping_option(', 'option deactivate handler'],
  ['pub async fn reactivate_shipping_option(', 'option reactivate handler'],
  ['list_shipping_profiles(', 'profile list service call'],
  ['create_shipping_profile(tenant.id, input)', 'profile create service call'],
  ['get_shipping_profile(', 'profile detail service call'],
  ['update_shipping_profile(tenant.id, id, input)', 'profile update service call'],
  ['deactivate_shipping_profile(tenant.id, id)', 'profile deactivate service call'],
  ['reactivate_shipping_profile(tenant.id, id)', 'profile reactivate service call'],
  ['list_all_shipping_options(', 'option list service call'],
  ['create_shipping_option(tenant.id, input)', 'option create service call'],
  ['get_shipping_option(', 'option detail service call'],
  ['update_shipping_option(tenant.id, id, input)', 'option update service call'],
  ['deactivate_shipping_option(tenant.id, id)', 'option deactivate service call'],
  ['reactivate_shipping_option(tenant.id, id)', 'option reactivate service call'],
  ['page: pagination.page', 'pagination page forwarding'],
  ['per_page: pagination.limit()', 'pagination size forwarding'],
  ['items.retain(|option| option.active == active)', 'active filter'],
  ['option.currency_code.eq_ignore_ascii_case(currency_code)', 'currency filter'],
  ['option.provider_id.eq_ignore_ascii_case(provider_id)', 'provider filter'],
  ['option.name.to_ascii_lowercase().contains(&search)', 'search filter'],
]) requireText(shipping, value, label);

for (const value of [
  'err.to_string()',
  'other.to_string()',
  'HttpError::bad_request("commerce_operation_failed"',
  'super::map_shipping_profile_error',
  'super::validate_shipping_option_profile_inputs',
  '.map_err(super::map_fulfillment_error)?;',
  'format!("Fulfillment request is invalid:',
]) forbidText(shipping, value, 'unsafe admin shipping public conversion');

const profileMapperUses = shipping.match(/map_shipping_profile_error\(/g) ?? [];
if (profileMapperUses.length !== 8) {
  failures.push(`expected profile mapper definition plus seven uses, found ${profileMapperUses.length}`);
}

const optionMapperUses =
  shipping.match(
    /map_admin_shipping_option_error\(\s+AdminShippingOptionErrorContext::new\(/g,
  ) ?? [];
if (optionMapperUses.length !== 6) {
  failures.push(`expected six context-aware shipping-option mapper callsites, found ${optionMapperUses.length}`);
}

const localValidationUses = shipping.match(/validate_shipping_option_profile_inputs\(/g) ?? [];
if (localValidationUses.length !== 3) {
  failures.push(`expected local validation helper definition plus two uses, found ${localValidationUses.length}`);
}

if (failures.length > 0) {
  console.error('Commerce admin shipping HTTP error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce admin shipping profile and option errors use stable context-aware public envelopes',
);
