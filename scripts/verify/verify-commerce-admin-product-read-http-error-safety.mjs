#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const products = read('crates/rustok-commerce/src/controllers/products.rs');
const adminProducts = read('crates/rustok-commerce/src/controllers/admin/products.rs');
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
  'fn map_product_service_error(error: CommerceError)',
  '/// Shared admin product list handler.',
  'shared admin product mapper',
);
const listHandler = between(
  products,
  'pub async fn list_products(',
  "fn pick_product_translation<'a>(",
  'product list handler',
);
const showHandler = between(
  products,
  'pub async fn show_product(',
  '/// Shared admin product delete handler.',
  'product detail handler',
);
const deleteHandler = between(
  products,
  'pub async fn delete_product(',
  '/// Shared admin product publish handler.',
  'product delete handler',
);
const publishHandler = between(
  products,
  'pub async fn publish_product(',
  '/// Shared admin product unpublish handler.',
  'product publish handler',
);
const unpublishHandler = between(
  products,
  'pub async fn unpublish_product(',
  '#[derive(Debug, serde::Deserialize, ToSchema, utoipa::IntoParams)]',
  'product unpublish handler',
);

for (const [value, label] of [
  ['CommerceError, dto::ProductResponse', 'typed commerce error import'],
  ['fn map_product_service_error(error: CommerceError)', 'typed product mapper'],
  ['fn map_product_database_error(error: sea_orm::DbErr)', 'direct database mapper'],
  ['map_product_service_error(CommerceError::Database(error))', 'database owner wrapping'],
  ['error = ?error', 'raw internal error logging'],
  ['owner = "rustok_product.catalog"', 'product owner logging'],
  ['error_kind,', 'error-kind logging'],
  ['public_code = code', 'public-code logging'],
  ['status = %status', 'status logging'],
  ['boundary = "commerce_admin_product_http"', 'product HTTP boundary logging'],
  ['HttpError::new(status, code, message)', 'static envelope construction'],
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
  ['CommerceError::ShippingProfileNotFound(_)', 'shipping-profile variant'],
  ['CommerceError::DuplicateShippingProfileSlug(_)', 'shipping-profile conflict variant'],
  ['CommerceError::NoVariants', 'no-variants variant'],
  ['CommerceError::CannotDeletePublished', 'published-delete variant'],
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
  ['"unexpected_owner_error"', 'unexpected owner kind'],
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
  forbidText(products, value, 'unsafe shared admin product public conversion');
}

for (const [value, label] of [
  ['Permission::PRODUCTS_LIST', 'product list permission'],
  ['product::Column::TenantId.eq(tenant.id)', 'tenant filter'],
  ['product::Column::Status.eq(status)', 'status filter'],
  ['product::Column::Vendor.eq(vendor)', 'vendor filter'],
  ['product::Column::ProductType.eq(product_type)', 'product type filter'],
  ['product_translation_title_search_condition(', 'localized search filter'],
  ['.offset(pagination.offset())', 'pagination offset'],
  ['.limit(pagination.limit())', 'pagination limit'],
  ['PaginationMeta::new(pagination.page, pagination.limit(), total)', 'pagination metadata'],
  ['metrics::record_read_path_query(', 'read query metrics'],
  ['metrics::record_read_path_budget(', 'read budget metrics'],
  ['.load_product_tag_map(', 'product tag service call'],
]) {
  requireText(listHandler, value, label);
}

const databaseMapperUses = products.match(/map_product_database_error/g) ?? [];
if (databaseMapperUses.length !== 4) {
  failures.push(
    `expected database mapper definition plus three list query callsites, found ${databaseMapperUses.length}`,
  );
}

for (const [content, values, label] of [
  [
    showHandler,
    [
      'Permission::PRODUCTS_READ',
      '.get_product_with_locale_fallback(',
      'tenant.id,',
      'request_context.locale.as_str()',
      'Some(tenant.default_locale.as_str())',
      '.map_err(map_product_service_error)?;',
      'Ok(Json(product))',
    ],
    'product detail path',
  ],
  [
    deleteHandler,
    [
      'Permission::PRODUCTS_DELETE',
      '.delete_product(tenant.id, auth.user_id, id)',
      '.map_err(map_product_service_error)?;',
      'Ok(StatusCode::NO_CONTENT)',
    ],
    'product delete path',
  ],
  [
    publishHandler,
    [
      'Permission::PRODUCTS_UPDATE',
      '.publish_product(tenant.id, auth.user_id, id)',
      '.map_err(map_product_service_error)?;',
      'Ok(Json(product))',
    ],
    'product publish path',
  ],
  [
    unpublishHandler,
    [
      'Permission::PRODUCTS_UPDATE',
      '.unpublish_product(tenant.id, auth.user_id, id)',
      '.map_err(map_product_service_error)?;',
      'Ok(Json(product))',
    ],
    'product unpublish path',
  ],
]) {
  for (const value of values) requireText(content, value, label);
}

const serviceMapperUses = products.match(/map_product_service_error/g) ?? [];
if (serviceMapperUses.length !== 7) {
  failures.push(
    `expected service mapper definition, database delegation, and five service callsites, found ${serviceMapperUses.length}`,
  );
}

for (const [value, label] of [
  ['super::super::products::list_products(', 'admin list delegation'],
  ['super::super::products::show_product(', 'admin detail delegation'],
  ['super::super::products::delete_product(', 'admin delete delegation'],
  ['super::super::products::publish_product(', 'admin publish delegation'],
  ['super::super::products::unpublish_product(', 'admin unpublish delegation'],
  ['fn map_product_write_error(error: CommerceError)', 'separate product write mapper'],
]) {
  requireText(adminProducts, value, label);
}

for (const [value, label] of [
  ['Database(#[from] sea_orm::DbErr)', 'owner database variant'],
  ['ProductNotFound(Uuid)', 'owner product variant'],
  ['VariantNotFound(Uuid)', 'owner variant variant'],
  ['DuplicateHandle { handle: String, locale: String }', 'owner handle variant'],
  ['DuplicateSku(String)', 'owner SKU variant'],
  ['InvalidPrice(String)', 'owner price variant'],
  ['InsufficientInventory { requested: i32, available: i32 }', 'owner inventory variant'],
  ['InvalidOptionCombination', 'owner option variant'],
  ['Validation(String)', 'owner validation variant'],
  ['ShippingProfileNotFound(Uuid)', 'owner shipping-profile variant'],
  ['DuplicateShippingProfileSlug(String)', 'owner profile-conflict variant'],
  ['NoVariants', 'owner no-variants variant'],
  ['CannotDeletePublished', 'owner state variant'],
  ['Rich(#[source] Box<RichError>)', 'owner rich variant'],
  ['Core(#[from] CoreError)', 'owner core variant'],
]) {
  requireText(commerceErrors, value, label);
}

for (const [value, label] of [
  ['pub async fn get_product_with_locale_fallback(', 'catalog detail operation'],
  ['pub async fn delete_product(', 'catalog delete operation'],
  ['pub async fn publish_product(', 'catalog publish operation'],
  ['pub async fn unpublish_product(', 'catalog unpublish operation'],
  ['CommerceError::ProductNotFound(product_id)', 'catalog not-found construction'],
  ['CommerceError::CannotDeletePublished', 'catalog state-conflict construction'],
  ['-> CommerceResult<ProductResponse>', 'typed product response result'],
]) {
  requireText(catalogService, value, label);
}

if (failures.length > 0) {
  console.error('Commerce admin product read/action HTTP error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Admin product list/detail/delete/publish errors use stable typed public envelopes');
