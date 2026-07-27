#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-mutation-storage] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const storePath = 'crates/rustok-index/src/infrastructure/postgres/mutation_store.rs';
const store = requireMarkers(storePath, [
  'pub struct MutationDelivery',
  'pub enum MutationApplyOutcome',
  'pub struct PostgresMutationStore',
  'registry.validate_mutation(delivery.mutation())?',
  'self.db.begin().await',
  'transaction.rollback().await',
  'ON CONFLICT (tenant_id, source_name, delivery_id) DO NOTHING',
  "state = 'applied'",
  'pg_advisory_xact_lock(hashtextextended($1, 0))',
  'FOR UPDATE',
  'incoming_source_version',
  'current_source_version',
  'WHERE excluded.source_version > index_entities.source_version',
  'DELETE FROM index_links WHERE tenant_id =',
  'INSERT INTO index_links',
  'DbBackend::Sqlite if cfg!(test) => Ok(())',
  'Decimal::from(source_version)',
  'SqliteSourceVersionOutOfRange',
]);

const lockPosition = store.indexOf('self.lock_entity_key(transaction, mutation, backend)');
const readPosition = store.indexOf('.current_source_version(transaction, mutation, backend)');
const deletePosition = store.indexOf('self.delete_existing_links(transaction, mutation, backend)');
const entityPosition = store.indexOf('.upsert_entity(');
if (
  lockPosition < 0 ||
  readPosition < 0 ||
  deletePosition < 0 ||
  entityPosition < 0 ||
  !(lockPosition < readPosition && readPosition < deletePosition && deletePosition < entityPosition)
) {
  fail('entity lock, version read, link delete, and entity upsert must remain ordered');
}

for (const forbidden of [
  'rustok_product',
  'rustok_content',
  'rustok_flex',
  'rustok_pricing',
  'rustok_inventory',
  'SELECT * FROM products',
]) {
  if (store.includes(forbidden)) fail(`${storePath} contains forbidden source-domain marker ${forbidden}`);
}


requireMarkers('crates/rustok-index/src/infrastructure/postgres/mutation_store_tests.rs', [
  'atomically_upserts_entity_links_and_terminal_inbox_state',
  'exact_redelivery_is_duplicate_but_payload_reuse_conflicts',
  'tombstone_and_source_version_guards_prevent_stale_resurrection',
  'failed_entity_write_rolls_back_the_inbox_claim',
  'tenant_and_locale_identity_do_not_collide',
]);

requireMarkers('crates/rustok-index/src/application/validation.rs', [
  'pub fn validate_mutation',
  'IndexMutation::Upsert',
  'IndexMutation::Delete',
  'validate_entity_key',
]);
requireMarkers('crates/rustok-index/src/lib.rs', [
  'pub mod infrastructure;',
  'PostgresMutationStore',
  'MutationDelivery',
  'MutationApplyOutcome',
]);
requireMarkers('crates/rustok-index/Cargo.toml', [
  'sea-orm.workspace = true',
  'serde_json.workspace = true',
]);

console.log('[verify-index-mutation-storage] OK');
