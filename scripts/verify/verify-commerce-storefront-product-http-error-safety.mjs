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
const commerceErrors = read('crates/rustok-commerce-foundation/src/error.rs');
const catalogService = read('crates/rustok-product/src/services/catalog.rs');
const catalogTags = read('crates/rustok-product/src/services/catalog/tags.rs');
const storefrontChannel = read('crates/rustok-commerce/src/storefront_channel.rs');
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

const productBoundary = between(
  controller,
  'fn map_storefront_product_error(',
  '/// List available storefront regions',
  'storefront product boundary',
);
const mapper = between(
  controller,
  'fn map_storefront_product_error(',
  '/// List published storefront products',
  'storefront product mapper',
);
const listHandler = between(
  controller,
  'pub async fn list_products(',
  '/// Show published storefront product',
  'storefront product list handler',
);
const showHandler = between(
  controller,
  'pub async fn show_product(',
  '/// List available storefront regions',
  'storefront product detail handler',
);

for (const [value, label] of [
  ['http::StatusCode', 'HTTP status import'],
  ['CommerceError,', 'typed commerce error import'],
  ['fn map_storefront_product_error(', 'typed storefront product mapper'],
  ['fn map_storefront_product_database_error(', 'direct database mapper'],
  ['CommerceError::Database(error)', 'database owner wrapping'],
  ['error = ?error', 'raw internal error logging'],
  ['operation,', 'operation logging'],
  ['tenant_id = %tenant_id', 'tenant logging'],
  ['product_id = ?product_id', 'optional product logging'],
  ['error_kind,', 'error-kind logging'],
  ['public_code = code', 'public-code logging'],
  ['status = %status', 'status logging'],
  ['boundary = "commerce_storefront_product_http"', 'product boundary logging'],
  ['HttpError::new(status, code, message)', 'static HTTP envelope construction'],
]) {
  requireText(controller, value, label);
}

for (const [value, label] of [
  ['CommerceError::Database(_)', 'database variant'],
  ['CommerceError::ProductNotFound(_)', 'product not-found variant'],
  ['CommerceError::VariantNotFound(_)', 'variant not-found variant'],
  ['CommerceError::Validation(_)', 'validation variant'],
  ['CommerceError::DuplicateHandle { .. }', 'duplicate handle variant'],
  ['CommerceError::DuplicateSku(_)', 'duplicate SKU variant'],
  ['CommerceError::InvalidPrice(_)', 'invalid price variant'],
  ['CommerceError::InsufficientInventory { .. }', 'inventory variant'],
  ['CommerceError::InvalidOptionCombination', 'invalid option variant'],
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
  ['StatusCode::BAD_REQUEST', 'validation status'],
  ['StatusCode::NOT_FOUND', 'not-found status'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'storage status'],
  ['StatusCode::INTERNAL_SERVER_ERROR', 'fail-closed status'],
  ['"commerce_store_product_invalid"', 'product invalid code'],
  ['"commerce_store_not_found"', 'storefront not-found code'],
  ['"commerce_store_product_unavailable"', 'product unavailable code'],
  ['"commerce_store_product_failed"', 'product fail-closed code'],
  ['"unexpected_owner_error"', 'unexpected owner kind'],
]) {
  requireText(mapper, value, label);
}

for (const value of [
  'commerce_operation_failed',
  'err.to_string()',
  'error.to_string()',
  'error.message',
  'error.code',
  'HttpError::bad_request(',
]) {
  forbidText(productBoundary, value, 'unsafe storefront product public conversion');
}

for (const [value, label] of [
  ['ensure_storefront_channel_enabled_for_db(', 'storefront channel guard'],
  ['let pagination = params.pagination.unwrap_or_default();', 'pagination input'],
  ['unwrap_or(request_context.locale.as_str())', 'locale fallback'],
  ['product::Column::TenantId.eq(tenant.id)', 'tenant filter'],
  ['product::Column::Status.eq(product::ProductStatus::Active)', 'active product filter'],
  ['product::Column::PublishedAt.is_not_null()', 'published product filter'],
  ['product::Column::Vendor.eq(vendor)', 'vendor filter'],
  ['product::Column::ProductType.eq(product_type)', 'product type filter'],
  ['product_translation_title_search_condition(', 'localized search filter'],
  ['.order_by_desc(product::Column::PublishedAt)', 'published ordering'],
  ['.order_by_desc(product::Column::CreatedAt)', 'created ordering'],
  ['is_metadata_visible_for_public_channel(', 'channel visibility filter'],
  ['.skip(pagination.offset() as usize)', 'pagination offset'],
  ['.take(pagination.limit() as usize)', 'pagination limit'],
  ['.load_product_tag_map(', 'product tag projection'],
  ['PaginationMeta::new(pagination.page, pagination.limit(), total)', 'pagination metadata'],
  ['"list_products"', 'list operation label'],
  ['"list_product_translations"', 'translation operation label'],
  ['"list_product_tags"', 'tag operation label'],
]) {
  requireText(listHandler, value, label);
}

for (const [value, label] of [
  ['ensure_storefront_channel_enabled_for_db(', 'detail channel guard'],
  ['.get_product_with_locale_fallback(', 'localized product read'],
  ['tenant.id,', 'detail tenant argument'],
  ['request_context.locale.as_str()', 'detail locale argument'],
  ['Some(tenant.default_locale.as_str())', 'detail fallback locale'],
  ['"show_product"', 'detail operation label'],
  ['product.status != product::ProductStatus::Active', 'active visibility check'],
  ['product.published_at.is_none()', 'published visibility check'],
  ['"commerce_store_not_found"', 'hidden product not-found code'],
  ['apply_public_channel_inventory_to_product(', 'public inventory projection'],
  ['public_channel_slug.as_deref()', 'public channel inventory argument'],
  ['"show_product_inventory"', 'inventory operation label'],
  ['Ok(Json(product))', 'detail response'],
]) {
  requireText(showHandler, value, label);
}

const serviceMapperUses = productBoundary.match(/map_storefront_product_error\(/g) ?? [];
if (serviceMapperUses.length !== 4) {
  failures.push(
    `expected service mapper definition, database delegation, tag callsite, and detail callsite, found ${serviceMapperUses.length}`,
  );
}
const databaseMapperUses = productBoundary.match(/map_storefront_product_database_error\(/g) ?? [];
if (databaseMapperUses.length !== 4) {
  failures.push(
    `expected database mapper definition plus product page, translations, and inventory callsites, found ${databaseMapperUses.length}`,
  );
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

for (const [content, value, label] of [
  [catalogService, 'pub async fn get_product_with_locale_fallback(', 'catalog detail operation'],
  [catalogService, '-> CommerceResult<ProductResponse>', 'typed catalog detail result'],
  [catalogService, 'CommerceError::ProductNotFound(product_id)', 'catalog not-found construction'],
  [catalogTags, 'pub async fn load_product_tag_map(', 'catalog tag operation'],
  [catalogTags, '-> CommerceResult<HashMap<Uuid, Vec<String>>>', 'typed tag result'],
  [catalogTags, 'CommerceError::Validation(error.to_string())', 'taxonomy error ownership'],
  [storefrontChannel, 'pub(crate) async fn apply_public_channel_inventory_to_product(', 'inventory projection operation'],
  [storefrontChannel, '-> Result<(), sea_orm::DbErr>', 'typed inventory database result'],
  [storefrontChannel, 'load_inventory_projection_by_variant_for_public_channel(', 'inventory owner call'],
]) {
  requireText(content, value, label);
}

for (const [value, label] of [
  ['pub async fn list_regions(', 'region handler retained'],
  ['pub async fn list_shipping_options(', 'shipping-options handler retained'],
]) {
  requireText(controller, value, label);
}

if (failures.length > 0) {
  console.error('Commerce storefront product HTTP error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Storefront product list and detail use stable typed public envelopes');
