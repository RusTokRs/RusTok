#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const admin = read('crates/rustok-commerce/src/controllers/admin/mod.rs');
const products = read('crates/rustok-commerce/src/controllers/admin/products.rs');
const commerceErrors = read('crates/rustok-commerce-foundation/src/error.rs');
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

const mapper = between(
  admin,
  'pub(crate) fn map_shipping_profile_error(error: crate::CommerceError)',
  'pub(crate) async fn validate_product_shipping_profile_input(',
  'shared shipping-profile mapper',
);
const productValidation = between(
  admin,
  'pub(crate) async fn validate_product_shipping_profile_input(',
  'pub(crate) async fn validate_shipping_option_profile_inputs(',
  'product shipping-profile validation helper',
);

for (const [value, label] of [
  ['let (status, code, message, error_kind) = match &error', 'typed exhaustive mapper'],
  ['admin_public_error(', 'shared structured public-error helper'],
  ['"rustok_commerce.shipping_profile"', 'shipping-profile owner log'],
  ['HttpError::new(status, code, message)', 'static envelope constructor'],
]) {
  requireText(admin, value, label);
}

for (const [value, label] of [
  ['crate::CommerceError::ShippingProfileNotFound(_)', 'profile not-found variant'],
  ['crate::CommerceError::DuplicateShippingProfileSlug(_)', 'duplicate profile slug variant'],
  ['crate::CommerceError::Validation(_)', 'profile validation variant'],
  ['crate::CommerceError::Database(_)', 'profile database variant'],
  ['axum::http::StatusCode::NOT_FOUND', 'not-found status'],
  ['axum::http::StatusCode::CONFLICT', 'conflict status'],
  ['axum::http::StatusCode::BAD_REQUEST', 'bad-request status'],
  ['axum::http::StatusCode::SERVICE_UNAVAILABLE', 'unavailable status'],
  ['axum::http::StatusCode::INTERNAL_SERVER_ERROR', 'fail-closed status'],
  ['"commerce_admin_not_found"', 'shared not-found code'],
  ['"commerce_admin_shipping_profile_conflict"', 'profile conflict code'],
  ['"commerce_admin_shipping_profile_invalid"', 'profile invalid code'],
  ['"commerce_admin_shipping_profile_storage_unavailable"', 'profile storage code'],
  ['"commerce_admin_shipping_profile_failed"', 'profile fail-closed code'],
  ['"unexpected_commerce_error"', 'unexpected owner variant kind'],
]) {
  requireText(mapper, value, label);
}

for (const value of [
  'crate::CommerceError::ProductNotFound(_)',
  'crate::CommerceError::VariantNotFound(_)',
  'crate::CommerceError::DuplicateHandle { .. }',
  'crate::CommerceError::DuplicateSku(_)',
  'crate::CommerceError::InvalidPrice(_)',
  'crate::CommerceError::InsufficientInventory { .. }',
  'crate::CommerceError::InvalidOptionCombination',
  'crate::CommerceError::NoVariants',
  'crate::CommerceError::CannotDeletePublished',
  'crate::CommerceError::Rich(_)',
  'crate::CommerceError::Core(_)',
]) {
  requireText(mapper, value, 'fail-closed unrelated commerce variant');
}

for (const value of [
  'other.to_string()',
  'error.to_string()',
  'commerce_admin_invalid',
  'commerce_operation_failed',
  'HttpError::bad_request(',
]) {
  forbidText(mapper, value, 'unsafe shared shipping-profile mapper conversion');
}

for (const [value, label] of [
  ['shipping_profile_slug.and_then(normalize_shipping_profile_slug)', 'slug normalization'],
  ['ShippingProfileService::new(db.clone())', 'owner service construction'],
  ['.ensure_shipping_profile_slug_exists(tenant_id, &slug)', 'owner validation call'],
  ['.map_err(map_shipping_profile_error)?;', 'typed mapper call'],
  ['return Ok(());', 'optional slug compatibility'],
]) {
  requireText(productValidation, value, label);
}

const productValidationUses = products.match(/super::validate_product_shipping_profile_input\(/g) ?? [];
if (productValidationUses.length !== 2) {
  failures.push(`expected create/update product validation callsites, found ${productValidationUses.length}`);
}
for (const [value, label] of [
  ['input.shipping_profile_slug.as_deref()', 'product shipping-profile slug forwarding'],
  ['create_product(tenant.id, auth.user_id, input)', 'product create service call'],
  ['update_product(tenant.id, auth.user_id, id, input)', 'product update service call'],
]) {
  requireText(products, value, label);
}

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
]) {
  requireText(commerceErrors, value, label);
}

for (const [value, label] of [
  ['pub async fn ensure_shipping_profile_slug_exists(', 'owner slug validation operation'],
  ['CommerceError::Validation("shipping profile slug is required".into())', 'empty slug validation'],
  ['"Unknown shipping profile slug: {slug}"', 'unknown slug validation'],
  ['.one(&self.db)', 'database-backed existence check'],
]) {
  requireText(shippingService, value, label);
}

if (failures.length > 0) {
  console.error('Commerce admin product shipping-profile HTTP error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Admin product shipping-profile validation uses stable typed public envelopes',
);
