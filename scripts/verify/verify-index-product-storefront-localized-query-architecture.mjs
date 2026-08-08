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
if (ownerQuery.slice(titleSearchStart).includes('pt.locale')) {
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
]);
if (productSource.includes('SchemaVersion::new(3)')) {
  fail(`${productSourcePath} must not restore historical Product routing key 3`);
}

const architecturePath = 'crates/rustok-index/docs/m7-product-storefront-localized-query-architecture.md';
requireMarkers(architecturePath, [
  'Status: `runtime_source_complete_text_pattern_and_evidence_pending`',
  '`localized_projection_fields`',
  '`SchemaRegistry::compile_postgres_localized_page_query`',
  '`SchemaRegistry::decode_postgres_localized_query_page`',
  '`IndexQueryPort` now publishes an explicit `execute_localized_query` method.',
  '`SharedIndexQueryRuntime` forwards the localized capability',
  '`PostgresIndexQueryPort::execute_localized_query` implements the canonical execution boundary',
  '`REPEATABLE READ, READ ONLY`',
  '`PostgresQueryEntityAdmission`',
  'The next source slice must add one generic bounded scalar string text-pattern primitive',
]);

requireMarkers('crates/rustok-index/docs/m7-product-storefront-parity-gate.md', [
  'Status: `localized_runtime_source_complete_text_pattern_adapter_and_evidence_pending`',
  'Storefront must continue to execute `CatalogService::list_published_products_with_query`',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-08.md', [
  'Status overlay rechecked from `main@6044dbd5110a65e2ebeee0a6a3a0053d9971b250` (#3210)',
  'Wire localized execution through the canonical PostgreSQL query runtime with persisted readiness,',
  'Add generic scalar string text-pattern matching inside folded `any_locale_filter`.',
  'Do not add a runtime alias for key `3`',
]);

console.log('[verify-index-product-storefront-localized-query-architecture] localized Product runtime architecture is locked; text pattern, adapter and evidence remain pending');
