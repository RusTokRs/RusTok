#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const carts = read('crates/rustok-commerce/src/controllers/store/carts.rs');
const boundary = read(
  'crates/rustok-commerce/src/controllers/store/line_item_resolution.rs',
);
const portContract = read('crates/rustok-api/src/ports.rs');
const commerceErrors = read('crates/rustok-commerce-foundation/src/error.rs');
const inventoryOwner = read('crates/rustok-inventory/src/services/public_channel.rs');
const productTests = read('crates/rustok-commerce/src/controllers/store/tests/products.rs');
const testRoot = read('crates/rustok-commerce/src/controllers/store/tests/mod.rs');
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
const from = (content, start, label) => {
  const startIndex = content.indexOf(start);
  if (startIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return content.slice(startIndex);
};

const databaseMapper = between(
  boundary,
  'fn map_storefront_line_item_database_error(',
  'fn map_storefront_line_item_pricing_error(',
  'line-item database mapper',
);
const pricingMapper = between(
  boundary,
  'fn map_storefront_line_item_pricing_error(',
  'fn map_storefront_line_item_inventory_error(',
  'line-item pricing mapper',
);
const inventoryMapper = between(
  boundary,
  'fn map_storefront_line_item_inventory_error(',
  'pub(super) async fn resolve_store_line_item_input(',
  'line-item inventory mapper',
);
const resolver = between(
  boundary,
  'pub(super) async fn resolve_store_line_item_input(',
  'pub(super) async fn validate_store_line_item_quantity(',
  'line-item resolver',
);
const quantityValidator = between(
  boundary,
  'pub(super) async fn validate_store_line_item_quantity(',
  'async fn validate_store_variant_inventory(',
  'line-item quantity validator',
);
const inventoryValidator = from(
  boundary,
  'async fn validate_store_variant_inventory(',
  'line-item inventory validator',
);

for (const [value, label] of [
  ['#[path = "line_item_resolution.rs"]', 'line-item module path'],
  ['mod line_item_resolution;', 'line-item module declaration'],
  ['line_item_resolution::resolve_store_line_item_input(', 'safe add-line resolver'],
  ['line_item_resolution::validate_store_line_item_quantity(', 'safe quantity validator'],
]) requireText(carts, value, label);
for (const value of [
  'super::resolve_store_line_item_input(',
  'super::validate_store_line_item_quantity(',
]) forbidText(carts, value, 'legacy unsafe production helper call');

for (const [value, label] of [
  ['use rustok_api::{PortContext, PortError};', 'typed pricing port imports'],
  ['DbErr', 'typed database error import'],
  ['CommerceError', 'typed inventory error import'],
  ['port_error_to_http_error', 'shared safe port mapper import'],
  ['boundary = "commerce_storefront_line_item_http"', 'line-item boundary name'],
]) requireText(boundary, value, label);

for (const [value, label] of [
  ['use crate::controllers::store::{ResolvedStoreLineItemInput, StoreLineItemResolution};', 'context-independent store imports'],
  ['crate::controllers::store::store_line_item_pricing_port_context(', 'absolute pricing context helper'],
  ['crate::controllers::store::storefront_cart_pricing_snapshot(', 'absolute pricing snapshot helper'],
  ['crate::controllers::store::pick_product_translation(', 'absolute product translation helper'],
  ['crate::controllers::store::pick_variant_translation(', 'absolute variant translation helper'],
  ['crate::controllers::store::merge_metadata(', 'absolute metadata merge helper'],
  ['crate::controllers::store::seller_snapshot_metadata(', 'absolute seller snapshot helper'],
]) requireText(boundary, value, label);
forbidText(boundary, 'super::super::', 'context-dependent line-item helper path');

for (const [value, label] of [
  ['#[path = "../line_item_resolution.rs"]', 'typed test module path'],
  ['mod line_item_resolution;', 'typed test module declaration'],
  ['use line_item_resolution::resolve_store_line_item_input;', 'typed test resolver import'],
  ['"commerce_store_inventory_insufficient"', 'typed inventory test code'],
  ['"Requested quantity is not available"', 'typed inventory test message'],
]) requireText(productTests, value, label);
for (const value of [
  '"commerce_store_invalid"',
  'does not have enough available inventory for the current channel',
]) forbidText(productTests, value, 'legacy inventory test contract');

forbidText(
  testRoot,
  'resolve_store_line_item_input,',
  'legacy line-item resolver test-root import',
);
requireText(
  testRoot,
  'StoreLineItemResolution,',
  'shared line-item resolution input test import',
);

for (const [value, label] of [
  ['owner = "rustok_product.persistence"', 'catalog persistence owner'],
  ['operation,', 'database operation logging'],
  ['tenant_id = %tenant_id', 'database tenant logging'],
  ['variant_id = ?variant_id', 'database variant logging'],
  ['product_id = ?product_id', 'database product logging'],
  ['error_kind = "database"', 'database error kind'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'database unavailable status'],
  ['"commerce_store_catalog_unavailable"', 'catalog unavailable code'],
  ['"Store catalog is temporarily unavailable"', 'static catalog message'],
  ['HttpError::new(status, code,', 'static database envelope'],
]) requireText(databaseMapper, value, label);

for (const [value, label] of [
  ['error: PortError', 'typed pricing port error'],
  ['context: &PortContext', 'pricing port context'],
  ['let public = port_error_to_http_error(error.clone());', 'shared safe pricing envelope'],
  ['owner = "rustok_pricing"', 'pricing owner logging'],
  ['operation = "resolve_product_price"', 'pricing operation logging'],
  ['tenant_id = %context.tenant_id', 'pricing tenant logging'],
  ['correlation_id = %context.correlation_id', 'pricing correlation logging'],
  ['channel = ?context.channel', 'pricing channel logging'],
  ['variant_id = %variant_id', 'pricing variant logging'],
  ['product_id = %product_id', 'pricing product logging'],
  ['error_kind = ?error.kind', 'pricing error-kind logging'],
  ['retryable = error.retryable', 'pricing retryability logging'],
  ['public_code = %public.code', 'pricing public-code logging'],
  ['status = %public.status', 'pricing status logging'],
  ['boundary = "commerce_storefront_line_item_http"', 'pricing boundary logging'],
  ['public\n}', 'pricing safe envelope return'],
]) requireText(pricingMapper, value, label);

for (const [value, label] of [
  ['CommerceError::Validation(_)', 'inventory validation variant'],
  ['CommerceError::ProductNotFound(_)', 'inventory product-not-found variant'],
  ['CommerceError::VariantNotFound(_)', 'inventory variant-not-found variant'],
  ['CommerceError::ShippingProfileNotFound(_)', 'inventory profile-not-found variant'],
  ['CommerceError::InsufficientInventory { .. }', 'inventory insufficient variant'],
  ['CommerceError::Database(_)', 'inventory database variant'],
  ['CommerceError::DuplicateHandle { .. }', 'unexpected duplicate handle variant'],
  ['CommerceError::DuplicateSku(_)', 'unexpected duplicate sku variant'],
  ['CommerceError::InvalidPrice(_)', 'unexpected invalid price variant'],
  ['CommerceError::InvalidOptionCombination', 'unexpected option variant'],
  ['CommerceError::DuplicateShippingProfileSlug(_)', 'unexpected profile conflict variant'],
  ['CommerceError::NoVariants', 'unexpected no-variants variant'],
  ['CommerceError::CannotDeletePublished', 'unexpected delete variant'],
  ['CommerceError::Rich(_)', 'unexpected rich variant'],
  ['CommerceError::Core(_)', 'unexpected core variant'],
  ['StatusCode::BAD_REQUEST', 'inventory invalid status'],
  ['StatusCode::NOT_FOUND', 'inventory not-found status'],
  ['StatusCode::SERVICE_UNAVAILABLE', 'inventory unavailable status'],
  ['StatusCode::INTERNAL_SERVER_ERROR', 'inventory fail-closed status'],
  ['"commerce_store_inventory_invalid"', 'inventory invalid code'],
  ['"commerce_store_not_found"', 'inventory not-found code'],
  ['"commerce_store_inventory_insufficient"', 'inventory insufficient code'],
  ['"commerce_store_inventory_unavailable"', 'inventory unavailable code'],
  ['"commerce_store_inventory_failed"', 'inventory fail-closed code'],
  ['owner = "rustok_inventory.public_channel"', 'inventory owner logging'],
  ['variant_id = %variant_id', 'inventory variant logging'],
  ['error_kind,', 'inventory error-kind logging'],
  ['public_code = code', 'inventory public-code logging'],
  ['status = %status', 'inventory status logging'],
  ['HttpError::new(status, code, message)', 'static inventory envelope'],
]) requireText(inventoryMapper, value, label);

for (const [value, label] of [
  ['product_variant::Entity::find_by_id(input.variant_id)', 'variant lookup'],
  ['product::Entity::find_by_id(variant.product_id)', 'product lookup'],
  ['product_translation::Entity::find()', 'product translation lookup'],
  ['variant_translation::Entity::find()', 'variant translation lookup'],
  ['"load_variant"', 'variant operation label'],
  ['"load_product"', 'product operation label'],
  ['"load_product_translations"', 'product translation operation label'],
  ['"load_variant_translations"', 'variant translation operation label'],
  ['product_model.status != product::ProductStatus::Active', 'active product guard'],
  ['product_model.published_at.is_none()', 'published product guard'],
  ['is_metadata_visible_for_public_channel', 'channel visibility guard'],
  ['let pricing_port_context =', 'pricing context retention'],
  ['store_line_item_pricing_port_context(', 'pricing context construction'],
  ['.resolve_product_price(', 'pricing resolution'],
  ['pricing_port_context.clone()', 'pricing context propagation'],
  ['map_storefront_line_item_pricing_error(', 'correlation-safe pricing mapper'],
  ['storefront_cart_pricing_snapshot', 'pricing snapshot'],
  ['validate_store_variant_inventory(', 'inventory validation'],
  ['pick_product_translation(', 'product title fallback'],
  ['pick_variant_translation(', 'variant title fallback'],
  ['effective_shipping_profile_slug(', 'shipping profile resolution'],
  ['seller_snapshot_metadata(', 'seller snapshot'],
  ['merge_metadata(', 'metadata merge'],
]) requireText(resolver, value, label);
forbidText(
  resolver,
  '.map_err(rustok_web::port_error_to_http_error)?',
  'unlogged pricing mapper call',
);

for (const [value, label] of [
  ['product_variant::Entity::find_by_id(variant_id)', 'quantity variant lookup'],
  ['"load_variant_for_quantity_validation"', 'quantity lookup operation'],
  ['validate_store_variant_inventory(', 'quantity inventory validation'],
  ['"commerce_store_not_found"', 'quantity not-found code'],
]) requireText(quantityValidator, value, label);

for (const [value, label] of [
  ['check_variant_availability_for_public_channel(', 'public-channel inventory check'],
  ['PublicChannelInventoryVariantProjectionInput {', 'typed inventory projection'],
  ['inventory_policy: &variant.inventory_policy', 'inventory policy propagation'],
  ['requested_quantity,', 'requested quantity propagation'],
  ['public_channel_slug,', 'public channel propagation'],
  ['"check_variant_availability"', 'inventory operation label'],
  ['map_storefront_line_item_inventory_error(', 'typed inventory mapper use'],
  ['if !available {', 'insufficient availability branch'],
  ['"commerce_store_inventory_insufficient"', 'stable insufficient code'],
  ['"Requested quantity is not available"', 'static insufficient message'],
]) requireText(inventoryValidator, value, label);

for (const value of [
  'err.to_string()',
  'error.to_string()',
  'HttpError::bad_request("commerce_store_invalid", error',
  'HttpError::bad_request("commerce_store_invalid", err',
]) forbidText(boundary, value, 'unsafe line-item public conversion');

for (const [content, value, label] of [
  [portContract, 'pub struct PortContext {', 'shared port context'],
  [portContract, 'pub correlation_id: String', 'shared correlation field'],
  [portContract, 'pub channel: Option<String>', 'shared channel field'],
  [portContract, 'pub struct PortError {', 'shared port error'],
  [portContract, 'pub kind: PortErrorKind', 'shared port error kind'],
  [portContract, 'pub retryable: bool', 'shared port retryability'],
  [commerceErrors, 'pub enum CommerceError {', 'commerce owner enum'],
  [commerceErrors, 'Database(#[from] sea_orm::DbErr)', 'commerce database variant'],
  [commerceErrors, 'ProductNotFound(Uuid)', 'commerce product variant'],
  [commerceErrors, 'VariantNotFound(Uuid)', 'commerce variant variant'],
  [commerceErrors, 'InsufficientInventory { requested: i32, available: i32 }', 'commerce inventory variant'],
  [commerceErrors, 'Validation(String)', 'commerce validation variant'],
  [commerceErrors, 'ShippingProfileNotFound(Uuid)', 'commerce profile variant'],
  [commerceErrors, 'Rich(#[source] Box<RichError>)', 'commerce rich variant'],
  [commerceErrors, 'Core(#[from] CoreError)', 'commerce core variant'],
  [inventoryOwner, '-> CommerceResult<bool>', 'inventory typed result'],
  [inventoryOwner, 'check_public_channel_inventory_request(', 'inventory request validation'],
  [inventoryOwner, 'load_available_inventory_for_variant_in_public_channel(', 'inventory storage read'],
  [inventoryOwner, 'Ok(available_inventory >= requested_quantity)', 'availability semantics'],
]) requireText(content, value, label);

const databaseMapperUses = boundary.match(/map_storefront_line_item_database_error\(/g) ?? [];
if (databaseMapperUses.length !== 6) {
  failures.push(`expected database mapper definition plus five uses, found ${databaseMapperUses.length}`);
}
const pricingMapperUses = boundary.match(/map_storefront_line_item_pricing_error\(/g) ?? [];
if (pricingMapperUses.length !== 2) {
  failures.push(`expected pricing mapper definition plus one use, found ${pricingMapperUses.length}`);
}
const inventoryMapperUses = boundary.match(/map_storefront_line_item_inventory_error\(/g) ?? [];
if (inventoryMapperUses.length !== 2) {
  failures.push(`expected inventory mapper definition plus one use, found ${inventoryMapperUses.length}`);
}

if (failures.length > 0) {
  console.error('Commerce storefront line-item HTTP error-safety verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log('✔ Storefront line-item catalog, pricing, and inventory errors use typed safe envelopes');
