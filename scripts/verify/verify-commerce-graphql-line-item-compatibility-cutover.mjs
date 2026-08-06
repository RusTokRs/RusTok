#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const facade = read('crates/rustok-commerce/src/graphql/mutations/safe_helpers.rs');
const typed = read(
  'crates/rustok-commerce/src/graphql/mutations/typed_line_item_helpers.rs',
);
const layered = read(
  'crates/rustok-commerce/src/graphql/mutations/layered_order_helpers.rs',
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

const resolveWrapper = between(
  facade,
  'pub(crate) async fn resolve_storefront_line_item_input(',
  'pub(crate) async fn reprice_storefront_cart_line_items(',
  'compatibility resolve wrapper',
);
const quantityStart = facade.indexOf(
  'pub(crate) async fn validate_storefront_line_item_quantity(',
);
const quantityWrapper = quantityStart >= 0 ? facade.slice(quantityStart) : '';
if (quantityStart < 0) failures.push('compatibility quantity wrapper: unable to isolate source block');

for (const [value, label] of [
  ['db: &sea_orm::DatabaseConnection', 'resolve database argument'],
  ['tenant_id: Uuid', 'resolve tenant argument'],
  ['pricing_read_port: &dyn PricingReadPort', 'resolve pricing port argument'],
  ['pricing_port_context: PortContext', 'resolve port context argument'],
  ['pricing_context: &PriceResolutionContext', 'resolve pricing context argument'],
  ['locale: &str', 'resolve locale argument'],
  ['default_locale: &str', 'resolve default-locale argument'],
  ['public_channel_slug: Option<&str>', 'resolve channel argument'],
  ['input: AddStorefrontCartLineItemInput', 'resolve input argument'],
  ['-> Result<ResolvedStorefrontLineItemInput>', 'resolve return type'],
  [
    'super::typed_line_item_helpers::resolve_storefront_line_item_input(',
    'resolve typed delegation',
  ],
  [
    `super::typed_line_item_helpers::resolve_storefront_line_item_input(
        db,
        tenant_id,
        pricing_read_port,
        pricing_port_context,
        pricing_context,
        locale,
        default_locale,
        public_channel_slug,
        input,
    )
    .await`,
    'resolve exact delegation arguments',
  ],
]) {
  requireText(resolveWrapper, value, label);
}

for (const [value, label] of [
  ['db: &sea_orm::DatabaseConnection', 'quantity database argument'],
  ['tenant_id: Uuid', 'quantity tenant argument'],
  ['variant_id: Uuid', 'quantity variant argument'],
  ['requested_quantity: i32', 'quantity amount argument'],
  ['public_channel_slug: Option<&str>', 'quantity channel argument'],
  ['-> Result<()>', 'quantity return type'],
  [
    'super::typed_line_item_helpers::validate_storefront_line_item_quantity(',
    'quantity typed delegation',
  ],
  [
    `super::typed_line_item_helpers::validate_storefront_line_item_quantity(
        db,
        tenant_id,
        variant_id,
        requested_quantity,
        public_channel_slug,
    )
    .await`,
    'quantity exact delegation arguments',
  ],
]) {
  requireText(quantityWrapper, value, label);
}

for (const value of [
  'super::legacy_helpers::resolve_storefront_line_item_input(',
  'super::legacy_helpers::validate_storefront_line_item_quantity(',
  '.map_err(',
  'legacy_graphql_error(',
  'format!("{error:?}")',
  'detail.contains(',
  'Variant not found',
  'Product not found',
  'does not have enough available inventory',
  'Invalid JSON metadata payload',
]) {
  forbidText(`${resolveWrapper}\n${quantityWrapper}`, value, 'compatibility string classifier');
}

for (const [pattern, expected, label] of [
  [
    /super::typed_line_item_helpers::resolve_storefront_line_item_input\(/g,
    1,
    'resolve typed call count',
  ],
  [
    /super::typed_line_item_helpers::validate_storefront_line_item_quantity\(/g,
    1,
    'quantity typed call count',
  ],
  [/\.map_err\(/g, 0, 'compatibility remap count'],
]) {
  const count = `${resolveWrapper}\n${quantityWrapper}`.match(pattern)?.length ?? 0;
  if (count !== expected) failures.push(`${label}: expected ${expected}, found ${count}`);
}

for (const [value, label] of [
  ['super::legacy_helpers::enrich_storefront_cart(', 'remaining enrichment compatibility call'],
  [
    'super::legacy_helpers::validate_selected_shipping_option(',
    'remaining shipping-option compatibility call',
  ],
  [
    'super::legacy_helpers::reprice_storefront_cart_line_items(',
    'remaining repricing compatibility call',
  ],
  ['pub(crate) use super::legacy_helpers::*;', 'remaining legacy symbol compatibility'],
]) {
  requireText(facade, value, label);
}
const remainingLegacyCalls = facade.match(/super::legacy_helpers::[a-z_]+\(/g) ?? [];
if (remainingLegacyCalls.length !== 3) {
  failures.push(`remaining legacy helper count: expected 3, found ${remainingLegacyCalls.length}`);
}

for (const [value, label] of [
  ['fn storefront_line_item_public_policy(', 'typed public policy'],
  ['"CART_PRODUCT_UNAVAILABLE"', 'typed unavailable code'],
  ['"CART_INVENTORY_INSUFFICIENT"', 'typed inventory code'],
  ['"CART_LINE_ITEM_INVALID"', 'typed invalid-input code'],
  ['"CART_LINE_ITEM_RESOLUTION_FAILED"', 'typed resolution fallback code'],
  ['"CART_INVENTORY_UNAVAILABLE"', 'typed quantity fallback code'],
  ['tracing::error!(', 'typed dependency severity'],
  ['tracing::warn!(', 'typed rejection severity'],
  ['struct StorefrontLineItemDiagnosticSource;', 'typed redacted source token'],
  ['formatter.write_str("redacted")', 'typed redacted source output'],
]) {
  requireText(typed, value, label);
}

for (const [value, label] of [
  [
    'pub(crate) use super::typed_line_item_helpers::{',
    'layered typed export owner',
  ],
  [
    'resolve_storefront_line_item_input, validate_storefront_line_item_quantity,',
    'layered typed exports',
  ],
]) {
  requireText(layered, value, label);
}
const layeredTypedExports = layered.match(
  /resolve_storefront_line_item_input|validate_storefront_line_item_quantity/g,
) ?? [];
if (layeredTypedExports.length !== 2) {
  failures.push(`layered typed export count: expected 2, found ${layeredTypedExports.length}`);
}

if (failures.length > 0) {
  console.error('Commerce GraphQL line-item compatibility cutover verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Commerce GraphQL line-item compatibility wrappers delegate directly to typed helpers without Debug-string classification or envelope remapping',
);
