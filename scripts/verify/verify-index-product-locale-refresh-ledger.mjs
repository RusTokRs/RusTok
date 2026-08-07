#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-locale-refresh-ledger] ${message}`);
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
  'crates/rustok-product/src/migrations/m20260806_000005_add_product_index_locale_refresh_ledger.rs';
const migration = requireMarkers(migrationPath, [
  'CREATE TABLE product_index_locale_refresh_ledger',
  'sequence_no BIGSERIAL NOT NULL',
  'refresh_id UUID NOT NULL',
  'root_event_id UUID NOT NULL',
  'UNIQUE (root_event_id, product_id, locale)',
  'UNIQUE (tenant_id, sequence_no)',
  'CHECK (source_version > 0)',
  'product Index locale refresh ledger is append-only',
  'BEFORE UPDATE ON product_index_locale_refresh_ledger',
  'BEFORE DELETE ON product_index_locale_refresh_ledger',
]);
if (migration.includes('REFERENCES products')) {
  fail(`${migrationPath} must retain hard-delete identities without a live Product foreign key`);
}

requireMarkers('crates/rustok-product/src/migrations/mod.rs', [
  'mod m20260806_000005_add_product_index_locale_refresh_ledger;',
  'Box::new(m20260806_000005_add_product_index_locale_refresh_ledger::Migration)',
]);

const sourcePath = 'crates/rustok-product/src/services/index_refresh.rs';
const source = requireMarkers(sourcePath, [
  'MAX_PRODUCT_INDEX_LOCALE_REFRESH_PAGE: usize = 256',
  'MAX_PRODUCT_INDEX_LOCALE_TARGETS_PER_EVENT: usize = 256',
  'pub struct ProductIndexLocaleRefreshRecord',
  'pub struct ProductIndexLocaleRefreshSource',
  'sequence_no > $2',
  'ORDER BY sequence_no ASC',
  'LIMIT $3',
  'pub(crate) fn product_locale_refresh_target',
  'DomainEvent::ProductCreated',
  'DomainEvent::ProductUpdated',
  'DomainEvent::ProductPublished',
  'DomainEvent::ProductDeleted',
  'product_translations translation',
  'products product',
  'product_index_tombstones tombstone',
  'NOT EXISTS',
  'product.index_revision AS source_version',
  'tombstone.source_version AS source_version',
  'MAX_PRODUCT_INDEX_LOCALE_TARGETS_PER_EVENT + 1',
  'INSERT INTO product_index_locale_refresh_ledger',
  'Uuid::new_v4()',
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
  'product_locale_refresh_target(&event)',
  'product_index_revision_touch_target(&event)',
  'DomainEvent::ProductAttributeValuesChanged { product_id } => Some(*product_id)',
  'let product_locale_id = lifecycle_product_id.or(product_attribute_id);',
  'let product_variant_id = lifecycle_product_id;',
  '.publish_in_tx_with_envelope_id(',
  'record_product_locale_refreshes_in_tx(',
  'record_product_variant_refreshes_in_tx(',
  'root_event_id',
  'Any source/ledger failure rolls',
  'back both the owner mutation and its event publication',
]);
const publishPosition = transaction.indexOf('.publish_in_tx_with_envelope_id(');
const recordPosition = transaction.indexOf('record_product_locale_refreshes_in_tx(');
if (publishPosition < 0 || recordPosition <= publishPosition) {
  fail(`${transactionPath} must retain the root event UUID before recording refresh rows`);
}

requireMarkers('crates/rustok-product/src/services/mod.rs', [
  'mod index_refresh;',
  'ProductIndexLocaleRefreshRecord',
  'ProductIndexLocaleRefreshSource',
]);
requireMarkers('crates/rustok-product/src/lib.rs', [
  'MAX_PRODUCT_INDEX_LOCALE_REFRESH_PAGE',
  'ProductIndexLocaleRefreshRecord',
  'ProductIndexLocaleRefreshSource',
]);

const cargo = read('crates/rustok-product/Cargo.toml');
if (cargo.includes('rustok-index')) {
  fail('rustok-product must not depend on rustok-index');
}

requireMarkers('crates/rustok-product/docs/index-locale-refresh-ledger.md', [
  'Status: `owner_source_complete_wire_and_consumer_pending`',
  '`refresh_id`, reserved as the future typed event and Index inbox identity',
  '`root_event_id`, the exact durable Product owner envelope',
  '`products.index_revision` remains the single Product owner input watermark',
  '`ProductAttributeValuesChanged` event first advances the Product Index clock',
  'final positive `products.index_revision`',
  '`product_index_tombstones.source_version`',
  '`ProductIndexLocaleRefreshSource::list` exposes at most 256 rows',
  'does not add or change a `rustok-events` wire family',
  'does not fabricate relation-convergence work',
  'No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI',
]);

console.log('[verify-index-product-locale-refresh-ledger] Product locale owner ledger contract verified');
