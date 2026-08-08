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
  'Status: `compiler_decoder_source_complete_runtime_and_evidence_pending`',
  '`localized_projection_fields`',
  '`SchemaRegistry::compile_postgres_localized_page_query`',
  '`t0` — deterministic admitted identity anchor',
  '`t1` — requested-locale projection row',
  '`t2` — fallback-locale projection row',
  '`t3` — any-locale existential predicate row',
  '`t4` — lower-locale anti-duplicate anchor candidate',
  '`SchemaRegistry::decode_postgres_localized_query_page`',
  '`LocalizedCursorCodec` uses scoped wire version `3`',
  'The public query runtime still has no `execute_localized_query` method.',
  'Wire `CompiledPostgresLocalizedPageQuery` into the PostgreSQL query port',
]);

requireMarkers('crates/rustok-index/docs/m7-product-storefront-parity-gate.md', [
  'Status: `localized_compiler_decoder_source_complete_runtime_and_evidence_pending`',
  'Storefront must continue to execute `CatalogService::list_published_products_with_query`',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-08.md', [
  'Status overlay rechecked from `main@678606a78916ed631669e40c617c60a031097138` (#3208)',
  'Compile the root-only localized-entity fold to one PostgreSQL page/exact-count contract.',
  'Add the localized page decoder with explicit requested/fallback-null semantics and dedicated cursor.',
  'wire explicit localized execution into the PostgreSQL Index query port',
  'Do not add a runtime alias for key `3`',
]);

console.log('[verify-index-product-storefront-localized-query-architecture] localized Product compiler/decoder architecture is locked; runtime and evidence remain pending');
