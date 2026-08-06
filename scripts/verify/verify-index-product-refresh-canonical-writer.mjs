#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-refresh-canonical-writer] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const writerPath = 'crates/rustok-product/src/services/index_refresh_publication.rs';
const writer = requireMarkers(writerPath, [
  'pub enum ProductIndexRefreshContractTarget',
  'Locale {',
  'Variant {',
  'pub trait ProductIndexRefreshContract: EventContract',
  'fn product_index_refresh_target(&self) -> ProductIndexRefreshContractTarget',
  'pub enum ProductIndexRefreshPublicationError',
  'ContractMismatch',
  'CausationMismatch',
  'Conflict',
  'Unavailable',
  'pub struct ProductIndexRefreshCanonicalWriter',
  'publish_locale_once_in_transaction',
  'publish_variant_once_in_transaction',
  'event.product_index_refresh_target() != expected',
  'load_product_root_actor(',
  'SysEvents::find_by_id(root_event_id)',
  'let envelope: EventEnvelope = serde_json::from_value(stored.payload)',
  '.validate_registered_schema()',
  'envelope.id != stored.id',
  'envelope.id != root_event_id',
  'envelope.event_type != stored.event_type',
  'envelope.tenant_id != tenant_id',
  'DomainEvent::ProductCreated',
  'DomainEvent::ProductUpdated',
  'DomainEvent::ProductPublished',
  'DomainEvent::ProductDeleted',
  'root_product_id != product_id',
  'Ok(envelope.actor_id)',
  'publish_contract_once_direct_in_tx_with_envelope_id_and_causation',
  'record.refresh_id()',
  'record.root_event_id()',
  'record.tenant_id()',
  'ContractEventWriteOnceError::Conflict',
  'ContractEventWriteOnceError::Unavailable',
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
  if (writer.includes(forbidden)) {
    fail(`${writerPath} contains forbidden runtime or untyped coupling: ${forbidden}`);
  }
}

const services = requireMarkers('crates/rustok-product/src/services/mod.rs', [
  'mod index_refresh_publication;',
  'ProductIndexRefreshCanonicalWriter',
  'ProductIndexRefreshContract',
  'ProductIndexRefreshContractTarget',
  'ProductIndexRefreshPublicationError',
]);
if (services.includes('pub mod index_refresh_publication')) {
  fail('the implementation module must remain private behind curated exports');
}

requireMarkers('crates/rustok-product/src/lib.rs', [
  'ProductIndexRefreshCanonicalWriter',
  'ProductIndexRefreshContract',
  'ProductIndexRefreshContractTarget',
  'ProductIndexRefreshPublicationError',
]);

const cargo = read('crates/rustok-product/Cargo.toml');
for (const dependency of ['rustok-events.workspace = true', 'rustok-outbox.workspace = true']) {
  if (!cargo.includes(dependency)) fail(`rustok-product is missing ${dependency}`);
}
if (cargo.includes('rustok-index')) {
  fail('rustok-product must not depend on rustok-index');
}

requireMarkers('crates/rustok-product/docs/index-refresh-canonical-writer.md', [
  'Status: `source_complete_typed_family_and_relay_pending`',
  '`refresh_id`, reserved as the canonical typed envelope',
  '`root_event_id`, the exact Product lifecycle predecessor',
  '`ProductIndexRefreshContract` extends the sealed',
  '`ProductIndexRefreshPublicationError::ContractMismatch`',
  '`ProductIndexRefreshPublicationError::CausationMismatch`',
  'reads `root_event_id` from canonical',
  'the root payload Product identity to match the ledger Product identity',
  'id = correlation_id = refresh_id',
  'causation_id = root_event_id',
  'actor_id = validated root envelope actor',
  'global `OutboxRelay`',
  'does not mutate the append-only Product refresh ledgers',
  'does not write Index tables',
  'change the event registry, transport schema or committed digests',
  'No tests, Node verifiers, Cargo checks',
]);

console.log('[verify-index-product-refresh-canonical-writer] Product refresh canonical writer contract verified');
