#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const boundary = read(
  'crates/rustok-commerce/src/controllers/store/line_item_resolution.rs',
);
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
  'database mapper',
);
const pricingMapper = between(
  boundary,
  'fn map_storefront_line_item_pricing_error(',
  'fn map_storefront_line_item_inventory_error(',
  'pricing mapper',
);
const inventoryMapper = between(
  boundary,
  'fn map_storefront_line_item_inventory_error(',
  'pub(crate) async fn resolve_store_line_item_input(',
  'inventory mapper',
);
const resolver = between(
  boundary,
  'pub(crate) async fn resolve_store_line_item_input(',
  'pub(crate) async fn validate_store_line_item_quantity(',
  'line item resolver',
);
const quantityValidator = between(
  boundary,
  'pub(crate) async fn validate_store_line_item_quantity(',
  'async fn validate_store_variant_inventory(',
  'quantity validator',
);
const inventoryValidator = from(
  boundary,
  'async fn validate_store_variant_inventory(',
  'inventory validator',
);

for (const [value, label] of [
  ['public_channel_slug: Option<&str>,', 'database channel parameter'],
  ['locale: Option<&str>,', 'database locale parameter'],
  ['owner = "rustok_product.persistence"', 'database owner'],
  ['operation,', 'database operation'],
  ['tenant_id = %tenant_id', 'database tenant'],
  ['variant_id = ?variant_id', 'database variant identity'],
  ['product_id = ?product_id', 'database product identity'],
  ['channel = ?public_channel_slug', 'database channel log'],
  ['locale = ?locale', 'database locale log'],
  ['error_kind = "database"', 'database error kind'],
  ['public_code = code', 'database public code'],
  ['status = %status', 'database status'],
  ['boundary = "commerce_storefront_line_item_http"', 'database boundary'],
  ['"commerce_store_catalog_unavailable"', 'database public code value'],
  ['"Store catalog is temporarily unavailable"', 'database public message'],
]) requireText(databaseMapper, value, label);

for (const [value, label] of [
  ['context: &PortContext', 'pricing port context'],
  ['correlation_id = %context.correlation_id', 'pricing correlation log'],
  ['channel = ?context.channel', 'pricing channel log'],
  ['variant_id = %variant_id', 'pricing variant identity'],
  ['product_id = %product_id', 'pricing product identity'],
  ['port_error_to_http_error(error.clone())', 'pricing public mapper'],
]) requireText(pricingMapper, value, label);

for (const [value, label] of [
  ['product_id: Uuid,', 'inventory product parameter'],
  ['public_channel_slug: Option<&str>,', 'inventory channel parameter'],
  ['locale: Option<&str>,', 'inventory locale parameter'],
  ['owner = "rustok_inventory.public_channel"', 'inventory owner'],
  ['variant_id = %variant_id', 'inventory variant identity'],
  ['product_id = %product_id', 'inventory product identity'],
  ['channel = ?public_channel_slug', 'inventory channel log'],
  ['locale = ?locale', 'inventory locale log'],
  ['CommerceError::Validation(_)', 'inventory validation variant'],
  ['CommerceError::ProductNotFound(_)', 'inventory product not found variant'],
  ['CommerceError::VariantNotFound(_)', 'inventory variant not found variant'],
  ['CommerceError::ShippingProfileNotFound(_)', 'inventory profile not found variant'],
  ['CommerceError::InsufficientInventory { .. }', 'inventory insufficient variant'],
  ['CommerceError::Database(_)', 'inventory database variant'],
  ['"commerce_store_inventory_invalid"', 'inventory invalid code'],
  ['"commerce_store_not_found"', 'inventory not found code'],
  ['"commerce_store_inventory_insufficient"', 'inventory insufficient code'],
  ['"commerce_store_inventory_unavailable"', 'inventory unavailable code'],
  ['"commerce_store_inventory_failed"', 'inventory fail closed code'],
  ['HttpError::new(status, code, message)', 'inventory stable envelope'],
]) requireText(inventoryMapper, value, label);

for (const [value, label] of [
  ['"load_variant"', 'variant database operation'],
  ['"load_product"', 'product database operation'],
  ['"load_product_translations"', 'product translation operation'],
  ['"load_variant_translations"', 'variant translation operation'],
  ['public_channel_slug,\n                Some(locale),', 'localized database context'],
  ['store_line_item_pricing_port_context(', 'pricing context construction'],
  ['pricing_port_context.clone()', 'pricing context forwarding'],
  ['map_storefront_line_item_pricing_error(', 'pricing mapper call'],
  ['validate_store_variant_inventory(', 'inventory validation call'],
  ['public_channel_slug,\n        Some(locale),', 'localized inventory context'],
  ['product_model.status != product::ProductStatus::Active', 'active product guard'],
  ['product_model.published_at.is_none()', 'published product guard'],
  ['is_metadata_visible_for_public_channel', 'channel visibility guard'],
  ['pick_product_translation(', 'product translation fallback'],
  ['pick_variant_translation(', 'variant translation fallback'],
  ['effective_shipping_profile_slug(', 'shipping profile resolution'],
  ['seller_snapshot_metadata(', 'seller metadata'],
  ['merge_metadata(', 'metadata merge'],
]) requireText(resolver, value, label);

for (const [value, label] of [
  ['"load_variant_for_quantity_validation"', 'quantity database operation'],
  ['public_channel_slug,\n                None,', 'quantity database context'],
  ['validate_store_variant_inventory(', 'quantity inventory validation'],
  ['public_channel_slug,\n        None,', 'quantity inventory context'],
  ['"commerce_store_not_found"', 'quantity not found code'],
]) requireText(quantityValidator, value, label);

for (const [value, label] of [
  ['locale: Option<&str>,', 'inventory validator locale'],
  ['check_variant_availability_for_public_channel(', 'inventory owner call'],
  ['variant_id: variant.id', 'inventory variant forwarding'],
  ['inventory_policy: &variant.inventory_policy', 'inventory policy forwarding'],
  ['requested_quantity,', 'requested quantity forwarding'],
  ['public_channel_slug,', 'public channel forwarding'],
  ['"check_variant_availability"', 'inventory operation'],
  ['variant.product_id,', 'inventory product identity forwarding'],
  ['locale,', 'inventory locale forwarding'],
  ['"Requested quantity is not available"', 'insufficient response message'],
]) requireText(inventoryValidator, value, label);

const databaseMapperUses =
  boundary.match(/map_storefront_line_item_database_error\(/g) ?? [];
if (databaseMapperUses.length !== 6) {
  failures.push(
    `expected database mapper definition plus five uses, found ${databaseMapperUses.length}`,
  );
}
const inventoryMapperUses =
  boundary.match(/map_storefront_line_item_inventory_error\(/g) ?? [];
if (inventoryMapperUses.length !== 2) {
  failures.push(
    `expected inventory mapper definition plus one use, found ${inventoryMapperUses.length}`,
  );
}
const localizedDatabaseContexts =
  resolver.match(/public_channel_slug,\s+Some\(locale\),/g) ?? [];
if (localizedDatabaseContexts.length !== 5) {
  failures.push(
    `expected four localized database contexts plus one inventory context, found ${localizedDatabaseContexts.length}`,
  );
}

for (const value of [
  'err.to_string()',
  'error.to_string()',
  'error.message',
  'HttpError::bad_request("commerce_store_invalid", error',
  'HttpError::bad_request("commerce_store_invalid", err',
  'eprintln!(',
  'dbg!(',
]) forbidText(boundary, value, 'unsafe line item mapping');

if (failures.length > 0) {
  console.error('Commerce storefront line-item owner-context verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Storefront line-item catalog and inventory failures retain channel, locale, and owner identities',
);
