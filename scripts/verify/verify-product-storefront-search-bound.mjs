#!/usr/bin/env node

/**
 * The Storefront product search bound is owned by Product and must be applied before
 * the owner builds any SQL from it, and the Index TextLike pattern must be able to
 * represent the bound plus the two LIKE wildcards exactly.
 *
 * Run: `node scripts/verify/verify-product-storefront-search-bound.mjs`
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? path.resolve(configuredRoot)
  : path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
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

const typesPath = 'crates/modules/rustok-product/src/services/catalog/types.rs';
const types = requireMarkers(typesPath, [
  'pub const MAX_STOREFRONT_PRODUCT_SEARCH_BYTES: usize = 1022;',
  'pub(crate) fn validate_storefront_product_search(',
  'search.len() > MAX_STOREFRONT_PRODUCT_SEARCH_BYTES',
  'search must contain at most {MAX_STOREFRONT_PRODUCT_SEARCH_BYTES} UTF-8 bytes',
  'search: normalize_storefront_product_search(search)?',
  'fn storefront_search_bound_uses_effective_utf8_bytes()',
]);

const ownerPath = 'crates/modules/rustok-product/src/services/catalog/queries.rs';
const owner = requireMarkers(ownerPath, [
  'pub async fn list_published_products_with_query(',
  'types::validate_storefront_product_search(list_query.search.as_deref())?;',
  'let pattern = format!("%{search}%");',
  'pt.title LIKE $1',
]);
const searchEntryPoint = 'pub async fn list_published_products_with_query(';
const searchEntryOffset = owner.indexOf(searchEntryPoint);
if (searchEntryOffset < 0) {
  fail(`${ownerPath} must own the Storefront search entry point ${searchEntryPoint.slice(0, -1)}`);
}

/*
 * The ordering check has to stay inside the one function that accepts a search term.
 * A file-wide `indexOf` compares the validation against the first product query in the
 * file, and that query belongs to `scan_published_product_ids` (a keyset scan with no
 * search input), so correct code failed the guard for an unrelated reason.
 */
const bodyStart = searchEntryOffset + searchEntryPoint.length;
const nextMethod = owner.slice(bodyStart).search(/\n {4}(?:pub(?:\([a-z]+\))? )?(?:async )?fn /);
const searchEntryBody = nextMethod < 0 ? owner.slice(bodyStart) : owner.slice(bodyStart, bodyStart + nextMethod);
const validationOffset = searchEntryBody.indexOf(
  'types::validate_storefront_product_search(list_query.search.as_deref())?;',
);
const queryOffset = searchEntryBody.indexOf(
  'let mut query = entities::product::Entity::find()',
);
if (validationOffset < 0) {
  fail(`${ownerPath} must validate the Storefront search inside ${searchEntryPoint.slice(0, -1)}`);
}
if (queryOffset < 0) {
  fail(
    `${ownerPath} must construct the Storefront product query inside ${searchEntryPoint.slice(0, -1)}`,
  );
}
if (queryOffset <= validationOffset) {
  fail(`${ownerPath} must validate the Storefront search before constructing owner SQL`);
}

const shadowPath = 'crates/modules/rustok-distribution/src/product_index/storefront_shadow.rs';
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

const indexValidationPath = 'crates/modules/rustok-index/src/application/validation.rs';
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
  'crates/modules/rustok-product/src/services/catalog.rs',
  'crates/modules/rustok-product/src/services/mod.rs',
]) {
  requireMarkers(relative, ['MAX_STOREFRONT_PRODUCT_SEARCH_BYTES']);
}

/*
 * An over-bound search has to be rejected, never shortened: truncating it turns the
 * user's query into a silent prefix match. The check is about truncation *of the search
 * value*, not about every `truncate` in these files — the owner pages its rows with
 * `rows.truncate(limit as usize)`, and a file-wide substring check failed the guard on
 * that unrelated line instead of on the contract.
 */
const searchBearingFiles = [
  [typesPath, types],
  [ownerPath, owner],
  [shadowPath, shadow],
];
for (const [relative, source] of searchBearingFiles) {
  for (const marker of [
    '.take(MAX_STOREFRONT_PRODUCT_SEARCH_BYTES)',
    'chars().take(MAX_STOREFRONT_PRODUCT_SEARCH_BYTES)',
  ]) {
    if (source.includes(marker)) {
      fail(`${relative} must reject over-bound Storefront search instead of truncating it: ${marker}`);
    }
  }
  for (const match of source.matchAll(/truncate\(/g)) {
    const lineStart = source.lastIndexOf('\n', match.index) + 1;
    const lineEnd = source.indexOf('\n', match.index);
    const line = source.slice(lineStart, lineEnd < 0 ? source.length : lineEnd);
    if (/search/i.test(line) || line.includes('MAX_STOREFRONT_PRODUCT_SEARCH_BYTES')) {
      fail(
        `${relative} must reject over-bound Storefront search instead of truncating it: ${line.trim()}`,
      );
    }
  }
}

console.log('[verify-product-storefront-search-bound] Product owns a 1022-byte Storefront search bound exactly representable by the 1024-byte Index TextLike pattern');
