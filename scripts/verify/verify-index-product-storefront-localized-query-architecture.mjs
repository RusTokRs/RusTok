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
  'Status: `source_decision_complete_implementation_and_evidence_pending`',
  'Keep the current owner Storefront behavior and keep exactly one current Product Index schema.',
  '`(tenant_id, schema_ref, entity_id)`',
  'requested locale',
  'fallback locale',
  'Any-locale title search is an identity predicate',
  'the row that satisfied search does **not** become the result locale merely because it matched',
  'requested-locale row',
  'fallback-locale row',
  'Grouping happens **before** pagination and exact count.',
  'exact count is the number of distinct admitted Product identities',
  'a cursor from an ordinary exact-locale query must not be accepted by the folded query path',
  'A consumer must not emulate this contract by issuing independent locale queries',
  'Existing schema readiness, Product entity freshness, and queried link-target availability remain',
  'Storefront remains owner-native.',
  'Implement the generic localized-entity fold in `rustok-index`',
]);

requireMarkers('crates/rustok-index/docs/m7-product-storefront-parity-gate.md', [
  'Status: `source_architecture_selected_implementation_and_evidence_pending`',
  'm7-product-storefront-localized-query-architecture.md',
  'A scalar substring/LIKE operator alone cannot close Storefront parity',
  'Storefront must continue to execute `CatalogService::list_published_products_with_query`',
]);

requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-08.md', [
  '`main@5f9e2fe4783d8bb5e6dad29adeff2dffc8296990`',
  'one current Product schema on internal routing key `4`',
  'Choose one generic localized Product query identity/fallback architecture',
  'implement the generic localized-entity fold in `rustok-index`',
  'Do not add a runtime alias for key `3`',
]);

console.log('[verify-index-product-storefront-localized-query-architecture] localized Product identity/fallback decision is locked; implementation and evidence remain pending');
