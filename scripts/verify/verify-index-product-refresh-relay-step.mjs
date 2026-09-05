#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-refresh-relay-step] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const relayPath = 'crates/rustok-product/src/services/index_refresh_relay.rs';
const relay = requireMarkers(relayPath, [
  'pub trait ProductIndexRefreshEventFactory: Send + Sync',
  'type LocaleEvent: ProductIndexRefreshContract',
  'type VariantEvent: ProductIndexRefreshContract',
  'pub enum ProductIndexRefreshRelayStepOutcome',
  'Idle { last_sequence_no: i64 }',
  'CursorAdvanced { last_sequence_no: i64 }',
  'Published { sequence_no: i64, refresh_id: Uuid }',
  'pub struct ProductIndexRefreshRelayStep<F>',
  'pub async fn publish_next_locale',
  'pub async fn publish_next_variant',
  '.list(tenant_id, observed_cursor, 1)',
  'lock_cursor(&transaction, tenant_id, LOCALE_STREAM_KIND)',
  'lock_cursor(&transaction, tenant_id, VARIANT_STREAM_KIND)',
  'ProductIndexRefreshCanonicalWriter::publish_locale_once_in_transaction',
  'ProductIndexRefreshCanonicalWriter::publish_variant_once_in_transaction',
  'advance_cursor(',
  'last_sequence_no = $3',
  'result.rows_affected() != 1',
  '.commit()',
  '.rollback()',
  'FOR UPDATE',
  'ON CONFLICT (tenant_id, stream_kind) DO NOTHING',
]);

for (const forbidden of [
  'serde_json::Value',
  'OutboxTransport::',
  'EventTransport::',
  'IndexMutation',
  'index_entities',
  'index_links',
  'tokio::spawn',
  'sleep(',
  'loop {',
  'acknowledge(',
  'publish_contract(',
]) {
  if (relay.includes(forbidden)) {
    fail(`${relayPath} contains forbidden runtime or untyped coupling: ${forbidden}`);
  }
}

const migrationPath =
  'crates/rustok-product/src/migrations/m20260806_000007_add_product_index_refresh_relay_cursors.rs';
requireMarkers(migrationPath, [
  'CREATE TABLE product_index_refresh_relay_cursors',
  'PRIMARY KEY (tenant_id, stream_kind)',
  "CHECK (stream_kind IN ('locale', 'variant'))",
  'CHECK (last_sequence_no >= 0)',
  'product Index refresh relay cursor identity is immutable',
  'product Index refresh relay cursor cannot move backwards',
  'product Index refresh relay cursor cannot be deleted',
  'BEFORE UPDATE ON product_index_refresh_relay_cursors',
  'BEFORE DELETE ON product_index_refresh_relay_cursors',
]);

requireMarkers('crates/rustok-product/src/migrations/mod.rs', [
  'mod m20260806_000007_add_product_index_refresh_relay_cursors;',
  'Box::new(m20260806_000007_add_product_index_refresh_relay_cursors::Migration)',
]);

const services = requireMarkers('crates/rustok-product/src/services/mod.rs', [
  'mod index_refresh_relay;',
  'ProductIndexRefreshEventFactory',
  'ProductIndexRefreshRelayError',
  'ProductIndexRefreshRelayStep',
  'ProductIndexRefreshRelayStepOutcome',
]);
if (services.includes('pub mod index_refresh_relay')) {
  fail('the relay implementation module must remain private behind curated exports');
}

requireMarkers('crates/rustok-product/src/lib.rs', [
  'ProductIndexRefreshEventFactory',
  'ProductIndexRefreshRelayError',
  'ProductIndexRefreshRelayStep',
  'ProductIndexRefreshRelayStepOutcome',
]);

requireMarkers('crates/rustok-product/docs/index-refresh-relay-step.md', [
  'Status: `source_complete_typed_family_source_ready_digest_regeneration_pending`',
  '`product_index_refresh_relay_cursors`',
  '`FOR UPDATE`',
  '`CursorAdvanced`',
  'outbox envelope id = outbox correlation id = ledger refresh_id',
  'outbox causation id = ledger root_event_id',
  'relay cursor = ledger sequence_no',
  '`ProductIndexRefreshEventFactory`',
  '`ProductIndexRefreshEvent` plus `CanonicalProductIndexRefreshEventFactory`',
  'global `OutboxRelay`',
  'No tests, Node verifiers, Cargo checks',
]);

const aggregate = read('scripts/verify/verify-index-query-contract.mjs');
if (!aggregate.includes("'verify-index-product-refresh-relay-step.mjs'")) {
  fail('Index aggregate verifier does not include the Product refresh relay step guard');
}

console.log('[verify-index-product-refresh-relay-step] Product refresh relay step contract verified');
