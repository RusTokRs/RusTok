#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-eav-owner-clock] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const valuesPath = 'crates/rustok-product/src/services/catalog_schema_service/values.rs';
requireMarkers(valuesPath, [
  'pub async fn save_product_attribute_values(',
  'pub async fn clear_detached_product_attribute_values(',
  'DomainEvent::ProductAttributeValuesChanged { product_id }',
]);

const transactionPath = 'crates/rustok-product/src/services/write_transaction.rs';
const transaction = requireMarkers(transactionPath, [
  'let product_attribute_id = product_index_revision_touch_target(&event);',
  'self.bump_product_index_revision(tenant_id, product_id)',
  'let lifecycle_product_id = product_locale_refresh_target(&event);',
  'let product_locale_id = lifecycle_product_id.or(product_attribute_id);',
  'let product_variant_id = lifecycle_product_id;',
  'DomainEvent::ProductAttributeValuesChanged { product_id } => Some(*product_id)',
  'DbBackend::Postgres',
  'UPDATE products SET index_revision = index_revision WHERE tenant_id = $1 AND id = $2',
  'result.rows_affected() != 1',
  'record_product_locale_refreshes_in_tx(',
  'record_product_variant_refreshes_in_tx(',
]);
const bumpPosition = transaction.indexOf('self.bump_product_index_revision(tenant_id, product_id)');
const publishPosition = transaction.indexOf('.publish_in_tx_with_envelope_id(');
const localePosition = transaction.indexOf('record_product_locale_refreshes_in_tx(');
if (bumpPosition < 0 || publishPosition <= bumpPosition || localePosition <= publishPosition) {
  fail(`${transactionPath} must bump Product source state before publishing and capture locale refresh after the durable root event id`);
}

requireMarkers('crates/rustok-product/src/migrations/m20260730_000001_add_product_index_revision.rs', [
  'CREATE TRIGGER trg_products_bump_index_revision',
  'NEW.index_revision := OLD.index_revision + 1',
  "RAISE EXCEPTION 'product index revision exhausted",
]);
requireMarkers('crates/rustok-product/src/migrations/m20260807_000010_canonicalize_product_index_graph_projection.rs', [
  'CREATE TRIGGER trg_products_index_graph_projection_update',
  'AFTER UPDATE OF index_revision ON products',
  'rustok_product_reconcile_index_graph_projection',
]);
requireMarkers('crates/rustok-product/src/migrations/m20260807_000012_add_product_sales_channel_relation_convergence.rs', [
  'CREATE TRIGGER trg_products_enqueue_channel_relation_convergence_update',
  'AFTER UPDATE OF metadata, tenant_id, id ON products',
]);

requireMarkers('crates/rustok-product/docs/index-locale-refresh-ledger.md', [
  'Product EAV value commands now use the same Product locale-refresh boundary',
  '`ProductAttributeValuesChanged` event first advances the Product Index clock',
  '`products.index_revision` remains the single Product owner input watermark',
  'do **not** fan the EAV command out to unchanged ProductVariant refresh rows',
  'does not fabricate relation-convergence work',
  'make Product EAV fields part of the current Index schema yet',
]);

console.log('[verify-index-product-eav-owner-clock] Product EAV changes advance the canonical Product clock and Product-only refresh boundary');
