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
  '.order_by_asc(entities::product::Column::Id)',
  '.order_by_desc(entities::product::Column::Id)',
  'let total = query.clone().count(&self.db).await?',
  'pick_product_translation(items.as_slice(), locale, fallback_locale)',
  'let pattern = format!("%{search}%");',
  'pt.title LIKE $1',
]);
const titleSearchStart = ownerQuery.indexOf('fn product_title_search_condition(');
if (titleSearchStart < 0) fail(`${ownerQueryPath} title-search helper is missing`);
if (ownerQuery.slice(titleSearchStart).includes('pt.locale')) {
  fail(`${ownerQueryPath} title search became locale-scoped`);
}

requireMarkers('crates/rustok-product/src/services/catalog/types.rs', [
  'pub struct StorefrontProductListQuery',
  'pub search: Option<String>',
  'search: normalize_optional_text(search)',
]);
requireMarkers('crates/rustok-distribution/src/product_index/mod.rs', [
  'PRODUCT_SCHEMA_ROUTING_KEY: u32 = 4',
  'Lower keys are historical storage identities only.',
]);

requireMarkers('crates/rustok-index/docs/m7-product-storefront-localized-query-architecture.md', [
  'Status: `runtime_text_pattern_identity_order_source_complete_adapter_and_evidence_pending`',
  '`identity_order_direction`',
  'entity_id DESC',
  'entity_id < cursor.entity_id',
  'Channel-less visibility',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan-current-2026-08-08.md', [
  'Status overlay rechecked at `main@',
  'localized identity fold, cursor v3 and requested -> fallback projection',
  'localized PostgreSQL compiler/decoder/runtime with readiness/admission and repeatable-read page/count snapshot',
  'Channel-less owner semantics are metadata-unrestricted only',
]);

console.log('[verify-index-product-storefront-localized-query-architecture] localized runtime/text-pattern/identity-order are locked; adapter and evidence remain pending');