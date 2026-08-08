#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-storefront-parity-gate] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const storefrontPath = 'crates/rustok-product/storefront/src/transport/catalog_list_native.rs';
const storefront = requireMarkers(storefrontPath, [
  'CatalogService::new(runtime_ctx.db_clone(), event_bus)',
  '.list_published_products_with_query(',
]);
for (const forbidden of ['rustok_index', 'SharedIndexQueryRuntime', 'IndexQuery']) {
  if (storefront.includes(forbidden)) fail(`${storefrontPath} contains forbidden marker ${forbidden}`);
}

const ownerPath = 'crates/rustok-product/src/services/catalog/queries.rs';
const owner = requireMarkers(ownerPath, [
  'ProductStatus::Active',
  'Column::PublishedAt.is_not_null()',
  'product_channel_visibility_condition(',
  'product_title_search_condition(',
  'let total = query.clone().count(&self.db).await?',
  'pick_product_translation(items.as_slice(), locale, fallback_locale)',
  'let pattern = format!("%{search}%");',
  'FROM product_translations pt',
  'pt.product_id = products.id',
  'pt.title LIKE $1',
]);
const titleSearch = owner.slice(owner.indexOf('fn product_title_search_condition('));
if (titleSearch.includes('pt.locale')) fail(`${ownerPath} title search became locale-scoped`);

requireMarkers('crates/rustok-product/src/services/catalog/types.rs', [
  'pub struct StorefrontProductListQuery',
  'pub search: Option<String>',
  'search: normalize_optional_text(search)',
]);

const productIndexPath = 'crates/rustok-distribution/src/product_index/product.rs';
const productIndex = requireMarkers(productIndexPath, [
  'assert_eq!(schema.fields.len(), 15);',
  'JOIN product_translations t',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
  'derive_index_schema_source_event_id(',
]);
if (productIndex.includes('SchemaVersion::new(3)')) fail(`${productIndexPath} restored historical key 3`);

requireMarkers('crates/rustok-index/docs/m7-product-storefront-parity-gate.md', [
  'Status: `localized_runtime_and_text_pattern_source_complete_adapter_and_evidence_pending`',
  'Generic `TextLike` — source complete',
  '`FilterExpr::TextLike(FieldPath, String)`',
  'no explicit search-length bound',
  'Storefront must continue to execute `CatalogService::list_published_products_with_query`',
]);
requireMarkers('crates/rustok-index/docs/m7-product-attribute-term-contract.md', [
  'Status: `source_complete_materialized_rebuild_pending`',
  '`requested-value OR (NOT requested-present AND fallback-value)`',
]);

console.log('[verify-index-product-storefront-parity-gate] Storefront remains owner-native while adapter/search-bound/collation/evidence gates are pending');