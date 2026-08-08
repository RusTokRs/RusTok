#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-product-storefront-search-bound] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const typesPath = 'crates/rustok-product/src/services/catalog/types.rs';
const types = requireMarkers(typesPath, [
  'pub const MAX_STOREFRONT_PRODUCT_SEARCH_BYTES: usize = 1022;',
  'pub(crate) fn validate_storefront_product_search(',
  'search.len() > MAX_STOREFRONT_PRODUCT_SEARCH_BYTES',
  'search must contain at most {MAX_STOREFRONT_PRODUCT_SEARCH_BYTES} UTF-8 bytes',
  'search: normalize_storefront_product_search(search)?',
  'fn storefront_search_bound_uses_effective_utf8_bytes()',
]);

const ownerPath = 'crates/rustok-product/src/services/catalog/queries.rs';
const owner = requireMarkers(ownerPath, [
  'pub async fn list_published_products_with_query(',
  'types::validate_storefront_product_search(list_query.search.as_deref())?;',
  'let pattern = format!("%{search}%");',
  'pt.title LIKE $1',
]);
const ownerValidation = owner.indexOf(
  'types::validate_storefront_product_search(list_query.search.as_deref())?;',
);
const ownerQuery = owner.indexOf('let mut query = entities::product::Entity::find()');
if (ownerValidation < 0 || ownerQuery <= ownerValidation) {
  fail(`${ownerPath} must validate the Storefront search before constructing owner SQL`);
}

const shadowPath = 'crates/rustok-distribution/src/product_index/storefront_shadow.rs';
const shadow = requireMarkers(shadowPath, [
  'services::MAX_STOREFRONT_PRODUCT_SEARCH_BYTES',
  'if search.len() > MAX_STOREFRONT_PRODUCT_SEARCH_BYTES',
  'let pattern = format!("%{search}%");',
  'FilterExpr::TextLike(root_field("title")?, pattern)',
  'fails_closed_when_public_query_fields_bypass_owner_search_constructor_bound',
]);
if (shadow.includes('const MAX_TEXT_LIKE_PATTERN_BYTES')) {
  fail(`${shadowPath} must consume the Product-owned search bound instead of owning another limit`);
}

const indexValidationPath = 'crates/rustok-index/src/application/validation.rs';
const indexValidation = requireMarkers(indexValidationPath, [
  'const MAX_TEXT_LIKE_PATTERN_BYTES: usize = 1024;',
]);

const ownerMatch = types.match(/MAX_STOREFRONT_PRODUCT_SEARCH_BYTES: usize = (\d+);/);
const indexMatch = indexValidation.match(/MAX_TEXT_LIKE_PATTERN_BYTES: usize = (\d+);/);
if (!ownerMatch || !indexMatch) fail('could not decode owner/Index search bounds');
const ownerBytes = Number(ownerMatch[1]);
const indexBytes = Number(indexMatch[1]);
if (ownerBytes + 2 !== indexBytes) {
  fail(`Product search bound ${ownerBytes} plus two LIKE wildcards does not equal Index TextLike bound ${indexBytes}`);
}

for (const relative of [
  'crates/rustok-product/src/services/catalog.rs',
  'crates/rustok-product/src/services/mod.rs',
]) {
  requireMarkers(relative, ['MAX_STOREFRONT_PRODUCT_SEARCH_BYTES']);
}

for (const forbidden of ['truncate(', '.take(MAX_STOREFRONT_PRODUCT_SEARCH_BYTES)', 'chars().take(']) {
  if (types.includes(forbidden) || owner.includes(forbidden) || shadow.includes(forbidden)) {
    fail(`Storefront search contract must reject over-bound input rather than truncate it: ${forbidden}`);
  }
}

console.log('[verify-product-storefront-search-bound] Product owns a 1022-byte Storefront search bound exactly representable by the 1024-byte Index TextLike pattern');
