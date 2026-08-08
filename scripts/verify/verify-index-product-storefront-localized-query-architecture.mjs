#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-storefront-localized-query-architecture] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const ownerQueryPath = 'crates/rustok-product/src/services/catalog/queries.rs';
const ownerQuery = requireMarkers(ownerQueryPath, [
  'pub async fn list_published_products_with_query(',
  'let total = query.clone().count(&self.db).await?',
  'pick_product_translation(items.as_slice(), locale, fallback_locale)',
  'fn product_title_search_condition(',
  'FROM product_translations pt',
  'pt.product_id = products.id',
  'pt.title LIKE $1',
]);
const titleSearchStart = ownerQuery.indexOf('fn product_title_search_condition(');
if (titleSearchStart < 0) fail(`${ownerQueryPath} title-search helper is missing`);
const titleSearch = ownerQuery.slice(titleSearchStart);
if (titleSearch.includes('pt.locale')) {
  fail(`${ownerQueryPath} title search became locale-scoped; revisit the localized identity architecture in the same PR`);
}

requireMarkers('crates/rustok-product/src/services/catalog/commands.rs', [
  'if input.translations.is_empty()',
  'At least one translation is required',
]);

requireMarkers('crates/rustok-distribution/src/product_index/mod.rs', [
  'PRODUCT_SCHEMA_ROUTING_KEY: u32 = 4',
  'Lower keys are historical storage identities only.',
]);
const productSourcePath = 'crates/rustok-distribution/src/product_index/product.rs';
const productSource = requireMarkers(productSourcePath, [
  'locale_mode: LocaleMode::Required',
  'JOIN product_translations t',
  't.locale',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
  'derive_index_schema_source_event_id(',
  'many_field("attribute_terms", IndexValueType::String, false, true)?',
  'name: link_name("variants")?',
  'name: link_name("sales_channels")?',
]);
if (productSource.includes('SchemaVersion::new(3)')) {
  fail(`${productSourcePath} must not restore historical Product routing key 3`);
}

const architecturePath = 'crates/rustok-index/docs/m7-product-storefront-localized-query-architecture.md';
requireMarkers(architecturePath, [
  'Status: `query_contract_source_complete_compiler_and_evidence_pending`',
  'Keep the current owner Storefront behavior and keep exactly one current Product Index schema.',
  '`(tenant_id, schema_ref, entity_id)`',
  '`LocalizedEntityQuery` wraps an ordinary validated `IndexQuery`',
  '`any_locale_filter` is a separate identity-level existential predicate',
  '`SchemaRegistry::validate_localized_entity_query` permits the mode only for `LocaleMode::Required`',
  '`LocalizedCursorCodec` and `LocalizedIndexCursor` provide a separate continuation identity',
  'wire version `3`',
  'ordinary exact-locale cursor wire version `2` remains unchanged',
  'Any-locale title search is an identity predicate',
  'the row that satisfied search does **not** become the result locale merely because it matched',
  'Grouping happens **before** pagination and exact count.',
  'exact count is the number of distinct admitted Product identities',
  'a cursor from an ordinary exact-locale query must not be accepted by the folded query path',
  'Existing schema readiness, Product entity freshness, and queried link-target availability remain',
  'Compile `LocalizedEntityQuery` into one PostgreSQL identity-fold statement',
]);

requireMarkers('crates/rustok-index/docs/m7-product-storefront-parity-gate.md', [
  'm7-product-storefront-localized-query-architecture.md',
  'A scalar substring/LIKE operator alone cannot close Storefront parity',
  'Storefront must continue to execute `CatalogService::list_published_products_with_query`',
]);

requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-08.md', [
  'Status overlay rechecked from `main@0a8d09a84688e4c0f3d6007d9b90d7f41b2a53a3` (#3204)',
  'one current Product schema on internal routing key `4`',
  'Add explicit generic localized query shape/validation',
  'Add dedicated localized cursor identity/version',
  'compile `LocalizedEntityQuery` into one PostgreSQL identity-fold page/count',
  'Do not add a runtime alias for key `3`',
]);

console.log('[verify-index-product-storefront-localized-query-architecture] localized Product architecture plus query/cursor contract are locked; compiler and evidence remain pending');
