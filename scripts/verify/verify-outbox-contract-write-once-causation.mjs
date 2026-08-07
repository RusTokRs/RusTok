#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-outbox-contract-write-once-causation] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const eventsPath = 'crates/rustok-events/src/contract.rs';
const events = requireMarkers(eventsPath, [
  'pub fn new_with_envelope_id_and_causation<E>(',
  'envelope_id: Uuid',
  'causation_id: Uuid',
  'Self::new_with_identity(',
  'Some(causation_id)',
  'explicit_contract_envelope_identity_and_causation_are_exact',
  'explicit_contract_envelope_identity_rejects_nil_causation_uuid',
]);
const constructorPosition = events.indexOf('pub fn new_with_envelope_id_and_causation<E>(');
const identityPosition = events.indexOf('Self::new_with_identity(', constructorPosition);
const causedPosition = events.indexOf('Some(causation_id)', identityPosition);
if (constructorPosition < 0 || identityPosition <= constructorPosition || causedPosition <= identityPosition) {
  fail(`${eventsPath} must delegate exact identity and causation to registered envelope construction`);
}

const outboxPath = 'crates/rustok-outbox/src/transactional.rs';
const outbox = requireMarkers(outboxPath, [
  'pub async fn publish_contract_once_direct_in_tx_with_envelope_id_and_causation<C, E>(',
  'ContractEventEnvelope::new_with_envelope_id_and_causation(',
  'OutboxTransport::write_contract_envelope_once_in_tx(txn, envelope).await',
  'ContractEventWriteOnceError::Unavailable',
]);
const publishPosition = outbox.indexOf(
  'pub async fn publish_contract_once_direct_in_tx_with_envelope_id_and_causation<C, E>(',
);
const buildPosition = outbox.indexOf(
  'ContractEventEnvelope::new_with_envelope_id_and_causation(',
  publishPosition,
);
const writePosition = outbox.indexOf(
  'OutboxTransport::write_contract_envelope_once_in_tx(txn, envelope).await',
  buildPosition,
);
if (publishPosition < 0 || buildPosition <= publishPosition || writePosition <= buildPosition) {
  fail(`${outboxPath} must construct the exact caused envelope before canonical write-once admission`);
}

const transportPath = 'crates/rustok-outbox/src/transport.rs';
const transport = requireMarkers(transportPath, [
  'stored.causation_id() != expected.causation_id()',
  'OnConflict::column(entity::Column::Id)',
  '.do_nothing()',
  'same_contract_publication(&stored_envelope, &envelope)',
]);
for (const forbidden of [
  'publish_contract_once_direct_in_tx_with_envelope_id_and_causation',
  'ContractEventEnvelope::new_with_envelope_id_and_causation',
]) {
  if (transport.includes(forbidden)) {
    fail(`${transportPath} must remain a generic canonical comparison boundary: ${forbidden}`);
  }
}

requireMarkers('crates/rustok-outbox/tests/contract_write_once.rs', [
  'exact_caused_replay_keeps_one_row_and_preserves_causation',
  'caused_write_once_rejects_causation_reuse_conflict',
  'assert_eq!(envelope.causation_id(), Some(root_event_id));',
  'ContractEventWriteOnceError::Conflict',
]);

requireMarkers('crates/rustok-outbox/docs/contract-write-once-causation.md', [
  'Status: `source_complete_owner_execution_pending`',
  '`refresh_id`, reserved as the typed envelope and Index inbox identity',
  '`root_event_id`, the exact Product lifecycle predecessor',
  'id = correlation_id = refresh_id',
  'causation_id = root_event_id',
  'does not add an event family',
  'No tests, Node verifiers, Cargo checks',
]);

for (const relative of [
  'crates/rustok-events/src/contract.rs',
  'crates/rustok-outbox/src/transactional.rs',
]) {
  const source = read(relative);
  for (const forbidden of [
    'product_index_locale_refresh',
    'product_variant_index_refresh',
    'ProductIndexLocaleRefresh',
    'ProductIndexVariantRefresh',
  ]) {
    if (source.includes(forbidden)) {
      fail(`${relative} must remain generic and contains Product-specific marker ${forbidden}`);
    }
  }
}

console.log('[verify-outbox-contract-write-once-causation] exact identity plus causation contract verified');
