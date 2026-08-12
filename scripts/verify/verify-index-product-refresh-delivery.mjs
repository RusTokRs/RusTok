#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-refresh-delivery] ${message}`);
  process.exit(1);
};
const requireMarker = (source, marker, label) => {
  if (!source.includes(marker)) fail(`${label} is missing ${marker}`);
};

const canonicalFamily = read('crates/rustok-events/src/product_index_refresh.rs');
const delivery = read('crates/rustok-distribution/src/product_index/refresh_event.rs');
const productIndexModule = read('crates/rustok-distribution/src/product_index/mod.rs');
const variantSource = read('crates/rustok-distribution/src/product_variant_index.rs');
const genericWorker = read('crates/rustok-index/src/application/source_refresh_event.rs');
const genericWorkerTests = read('crates/rustok-index/src/application/source_refresh_event_tests.rs');

for (const eventDomain of [
  'product.index.locale_refresh_requested',
  'product.index.variant_refresh_requested',
]) {
  requireMarker(canonicalFamily, eventDomain, 'canonical Product refresh family');
  requireMarker(delivery, eventDomain, 'distribution Product refresh delivery');
}

for (const marker of [
  'register_index_mutation_event(',
  'PRODUCT_INDEX_SOURCE',
  'PRODUCT_VARIANT_INDEX_SOURCE',
  'ModuleName::new("rustok-product")',
  'EntityName::new("product")',
  'EntityName::new("product_variant")',
  'SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY)',
  'SchemaVersion::new(PRODUCT_VARIANT_SCHEMA_VERSION)',
  'LocaleKey::new(locale)',
  'entity_id: product_id',
  'entity_id: variant_id',
  'locale: None',
  'IndexSourceRefreshEventDelivery::new(',
  'IndexSourceRefreshEventWorker::new(',
  '.into_index_delivery()?',
]) {
  requireMarker(delivery, marker, 'distribution Product refresh delivery');
}

requireMarker(productIndexModule, 'pub(crate) const PRODUCT_SCHEMA_ROUTING_KEY: u32 = 4;', 'Product Index module');
requireMarker(productIndexModule, 'refresh_event::register(extensions)?;', 'Product Index module');
requireMarker(variantSource, 'const PRODUCT_VARIANT_SCHEMA_VERSION: u32 = 2;', 'ProductVariant Index source');
requireMarker(delivery, 'const PRODUCT_VARIANT_SCHEMA_VERSION: u32 = 2;', 'distribution Product refresh delivery');

const applyPosition = genericWorker.indexOf('.apply_replay_mutation(');
const acknowledgePosition = genericWorker.indexOf('.acknowledge(');
if (applyPosition === -1 || acknowledgePosition === -1 || applyPosition > acknowledgePosition) {
  fail('generic source refresh worker no longer preserves durable apply-before-ack ordering');
}

for (const marker of [
  'canonical_source_mutation_is_rebound_committed_and_then_acknowledged',
  'missing_or_behind_source_state_suppresses_apply_and_ack',
  'schema_mismatch_fails_before_source_load_apply_or_ack',
]) {
  requireMarker(genericWorkerTests, marker, 'generic source refresh worker tests');
}

for (const forbidden of [
  'rustok-product.product-replay',
  'rustok-product.product-variant-replay',
  'product.index.product-locale-refresh-v1',
  'fallback',
  'legacy',
]) {
  if (delivery.includes(forbidden)) {
    fail(`distribution Product refresh delivery contains forbidden compatibility route: ${forbidden}`);
  }
}

console.log('[verify-index-product-refresh-delivery] typed Product/ProductVariant refresh delivery verified');
