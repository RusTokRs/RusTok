#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-variant-refresh-ledger] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const migrationPath =
  'crates/rustok-product/src/migrations/m20260806_000006_add_product_variant_index_refresh_ledger.rs';
const migration = requireMarkers(migrationPath, [
  'ALTER TABLE product_variant_index_tombstones',
  'ADD COLUMN product_id UUID NULL',
  'WHERE product_id IS NOT NULL',
  'CREATE TABLE product_variant_index_refresh_ledger',
  'sequence_no BIGSERIAL NOT NULL',
  'UNIQUE (root_event_id, variant_id)',
  'UNIQUE (tenant_id, sequence_no)',
  'product variant Index refresh ledger is append-only',
  'BEFORE UPDATE ON product_variant_index_refresh_ledger',
  'BEFORE DELETE ON product_variant_index_refresh_ledger',
  'target_product_id UUID',
  'OLD.product_id',
  'DROP FUNCTION rustok_product_variant_store_index_tombstone(UUID, UUID, BIGINT)',
]);
if (migration.includes('REFERENCES products')) {
  fail(`${migrationPath} must preserve hard-delete identities without a live Product foreign key`);
}

requireMarkers('crates/rustok-product/src/migrations/mod.rs', [
  'mod m20260806_000006_add_product_variant_index_refresh_ledger;',
  'Box::new(m20260806_000006_add_product_variant_index_refresh_ledger::Migration)',
]);

const sourcePath = 'crates/rustok-product/src/services/index_refresh.rs';
const source = requireMarkers(sourcePath, [
  'MAX_PRODUCT_INDEX_VARIANT_REFRESH_PAGE: usize = 256',
  'pub struct ProductIndexVariantRefreshRecord',
  'pub struct ProductIndexVariantRefreshSource',
  'FROM product_variant_index_refresh_ledger',
  'sequence_no > $2',
  'ORDER BY sequence_no ASC',
  'LIMIT $3',
  'record_product_variant_refreshes_in_tx',
  'product_variants variant',
  'product_variant_index_tombstones tombstone',
  'tombstone.product_id = $2',
  'NOT EXISTS',
  'variant.index_revision AS source_version',
  'tombstone.source_version AS source_version',
  'md5(',
  'INSERT INTO product_variant_index_refresh_ledger',
  'Historical parentless tombstones remain replayable',
]);
for (const forbidden of [
  'rustok_index',
  'IndexMutation',
  'index_entities',
  'index_links',
  'tokio::spawn',
  'sleep(',
]) {
  if (source.includes(forbidden)) {
    fail(`${sourcePath} contains forbidden Index/runtime coupling: ${forbidden}`);
  }
}

const transactionPath = 'crates/rustok-product/src/services/write_transaction.rs';
const transaction = requireMarkers(transactionPath, [
  '.publish_in_tx_with_envelope_id(',
  'record_product_locale_refreshes_in_tx(',
  'record_product_variant_refreshes_in_tx(',
  'root_event_id',
  'rolls back both the owner mutation and its event publication',
  'The same atomic boundary includes both refresh ledgers',
]);
const rootPosition = transaction.indexOf('.publish_in_tx_with_envelope_id(');
const localePosition = transaction.indexOf('record_product_locale_refreshes_in_tx(');
const variantPosition = transaction.indexOf('record_product_variant_refreshes_in_tx(');
if (
  rootPosition < 0 ||
  localePosition <= rootPosition ||
  variantPosition <= localePosition
) {
  fail(`${transactionPath} must publish root, record locale, then record variant rows`);
}

requireMarkers('crates/rustok-product/src/services/mod.rs', [
  'MAX_PRODUCT_INDEX_VARIANT_REFRESH_PAGE',
  'ProductIndexVariantRefreshRecord',
  'ProductIndexVariantRefreshSource',
]);
requireMarkers('crates/rustok-product/src/lib.rs', [
  'MAX_PRODUCT_INDEX_VARIANT_REFRESH_PAGE',
  'ProductIndexVariantRefreshRecord',
  'ProductIndexVariantRefreshSource',
]);

const cargo = read('crates/rustok-product/Cargo.toml');
if (cargo.includes('rustok-index')) {
  fail('rustok-product must not depend on rustok-index');
}

requireMarkers('crates/rustok-product/docs/index-variant-refresh-ledger.md', [
  'Status: `owner_source_complete_wire_and_consumer_pending`',
  'Historical tombstones created before this migration keep `product_id = NULL`',
  '`refresh_id` derived from tenant, Product, root event and variant identity',
  '`product_variants.index_revision`',
  '`product_variant_index_tombstones.source_version`',
  '`ProductIndexVariantRefreshSource::list` returns at most 256 rows',
  'does not impose a new maximum variant count',
  'does not add or change a `rustok-events` wire family',
  'No tests, Node verifiers, Cargo checks',
]);

console.log('[verify-index-product-variant-refresh-ledger] ProductVariant owner ledger contract verified');
