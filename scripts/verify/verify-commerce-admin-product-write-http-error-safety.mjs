#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const products = read('crates/rustok-commerce/src/controllers/admin/products.rs');
const admin = read('crates/rustok-commerce/src/controllers/admin/mod.rs');
const commerceErrors = read('crates/rustok-commerce-foundation/src/error.rs');
const catalogService = read('crates/rustok-product/src/services/catalog.rs');
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
  products,
  'fn map_product_write_error(error: CommerceError)',
  '/// List admin ecommerce products',
  'admin product write mapper',
);
const createHandler = between(
  products,
  'pub async fn create_product(',
  '/// Show admin ecommerce product',
  'admin product create handler',
);
const updateHandler = between(
  products,
  'pub async fn update_product(',
  '/// Delete admin ecommerce product',
  'admin product update handler',
);

for (const [value, label] of [
  ['CommerceError,', 'typed commerce error import'],
  ['fn map_product_write_error(error: CommerceError)', 'local product write mapper'],
  ['let (status, code, message, error_kind) = match &error', 'typed exhaustive mapping'],
  ['super::admin_public_error(', 'shared structured public-error helper'],
  ['"rustok_product.catalog"', 'catalog owner log'],
]) {
  requireText(products, value, label);
}

for (const [value, label] of [
  ['CommerceError::Database(_)', 'database variant'],
  ['CommerceError::ProductNotFound(_)', 'product not-found variant'],
  ['CommerceError::VariantNotFound(_)', 'variant not-found variant'],
  ['CommerceError::DuplicateHandle { .. }', 'duplicate handle variant'],
  ['CommerceError::DuplicateSku(_)', 'duplicate SKU variant'],
  ['CommerceError::InvalidPrice(_)', 'invalid price variant'],
  ['CommerceError::InsufficientInventory { .. }', 'inventory variant'],
  ['CommerceError::InvalidOptionCombination', 'invalid option variant'],
  ['CommerceError::Validation(_)', 'validation variant'],
  ['CommerceError::ShippingProfileNotFound(_)', 'unexpected shipping-profile variant'],
  ['CommerceError::DuplicateShippingProfileSlug(_)', 'unexpected shipping-profile conflict variant'],
  ['CommerceError::NoVariants', 'no variants variant'],
  ['CommerceError::CannotDeletePublished', 'published product state variant'],
  ['CommerceError::Rich(_)', 'rich error variant'],
  ['CommerceError::Core(_)', 'core error variant'],
]) {
  requireText(mapper, value, label);
}

for (const [value, label] of [
  ['StatusCode::BAD_REQUEST', 'bad-request status'],
  ['StatusCode::NOT_FOUND', 'not-found status'],
  ['StatusCode::CONFLICT', 'conflict status'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'storage unavailable status'],
  ['StatusCode::INTERNAL_SERVER_ERROR', 'fail-closed status'],
  ['"commerce_admin_not_found"', 'shared not-found code'],
  ['"commerce_admin_product_handle_conflict"', 'handle conflict code'],
  ['"commerce_admin_product_sku_conflict"', 'SKU conflict code'],
  ['"commerce_admin_product_invalid"', 'product invalid code'],
  ['"commerce_admin_product_inventory_conflict"', 'inventory conflict code'],
  ['"commerce_admin_product_state_conflict"', 'state conflict code'],
  ['"commerce_admin_product_storage_unavailable"', 'storage unavailable code'],
  ['"commerce_admin_product_failed"', 'fail-closed code'],
  ['"unexpected_owner_error"', 'unexpected owner error kind'],
]) {
  requireText(mapper, value, label);
}

for (const value of [
  'err.to_string()',
  'error.to_string()',
  'other.to_string()',
  'commerce_operation_failed',
  'HttpError::bad_request(',
]) {
  forbidText(products, value, 'unsafe admin product write public conversion');
}

for (const [content, values, label] of [
  [
    createHandler,
    [
      'Permission::PRODUCTS_CREATE',
      'super::validate_product_shipping_profile_input(',
      'input.shipping_profile_slug.as_deref()',
      'CatalogService::new(runtime.db_clone(), runtime.event_bus())',
      '.create_product(tenant.id, auth.user_id, input)',
      '.map_err(map_product_write_error)?;',
      'Ok((StatusCode::CREATED, Json(product)))',
    ],
    'product create path',
  ],
  [
    updateHandler,
    [
      'Permission::PRODUCTS_UPDATE',
      'super::validate_product_shipping_profile_input(',
      'input.shipping_profile_slug.as_deref()',
      'CatalogService::new(runtime.db_clone(), runtime.event_bus())',
      '.update_product(tenant.id, auth.user_id, id, input)',
      '.map_err(map_product_write_error)?;',
      'Ok(Json(product))',
    ],
    'product update path',
  ],
]) {
  for (const value of values) requireText(content, value, label);
}

for (const [value, label] of [
  ['pub async fn list_products(', 'list handler'],
  ['super::super::products::list_products(', 'list delegation'],
  ['pub async fn show_product(', 'show handler'],
  ['super::super::products::show_product(', 'show delegation'],
  ['pub async fn delete_product(', 'delete handler'],
  ['super::super::products::delete_product(', 'delete delegation'],
  ['pub async fn publish_product(', 'publish handler'],
  ['super::super::products::publish_product(', 'publish delegation'],
  ['pub async fn unpublish_product(', 'unpublish handler'],
  ['super::super::products::unpublish_product(', 'unpublish delegation'],
]) {
  requireText(products, value, label);
}

const mapperUses = products.match(/map_product_write_error/g) ?? [];
if (mapperUses.length !== 3) {
  failures.push(`expected mapper definition plus two write callsites, found ${mapperUses.length}`);
}
const shippingValidationUses =
  products.match(/super::validate_product_shipping_profile_input\(/g) ?? [];
if (shippingValidationUses.length !== 2) {
  failures.push(`expected two product shipping-profile validation callsites, found ${shippingValidationUses.length}`);
}

for (const [value, label] of [
  ['error = ?error', 'raw internal error logging'],
  ['owner,', 'owner logging'],
  ['error_kind,', 'error-kind logging'],
  ['public_code = code', 'public-code logging'],
  ['status = %status', 'status logging'],
  ['boundary = "commerce_admin_http"', 'admin HTTP boundary logging'],
  ['HttpError::new(status, code, message)', 'static public envelope construction'],
]) {
  requireText(admin, value, label);
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
  ['ShippingProfileNotFound(Uuid)', 'owner shipping-profile variant'],
  ['DuplicateShippingProfileSlug(String)', 'owner shipping-profile conflict variant'],
  ['NoVariants', 'owner no-variants variant'],
  ['CannotDeletePublished', 'owner state variant'],
  ['Rich(#[source] Box<RichError>)', 'owner rich variant'],
  ['Core(#[from] CoreError)', 'owner core variant'],
]) {
  requireText(commerceErrors, value, label);
}

for (const [value, label] of [
  ['pub async fn create_product(', 'catalog create operation'],
  ['pub async fn update_product(', 'catalog update operation'],
  ['-> CommerceResult<ProductResponse>', 'typed catalog result'],
  ['CommerceError::Validation(e.to_string())', 'catalog validation construction'],
  ['CommerceError::NoVariants', 'catalog no-variants construction'],
  ['CommerceError::DuplicateHandle {', 'catalog duplicate-handle construction'],
  ['CommerceError::DuplicateSku(', 'catalog duplicate-SKU construction'],
  ['CommerceError::ProductNotFound(product_id)', 'catalog not-found construction'],
  ['map_product_unique_violation(', 'database conflict classification'],
  ['ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;', 'database propagation'],
]) {
  requireText(catalogService, value, label);
}

if (failures.length > 0) {
  console.error('Commerce admin product write HTTP error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Admin product create/update errors use stable typed public envelopes');
