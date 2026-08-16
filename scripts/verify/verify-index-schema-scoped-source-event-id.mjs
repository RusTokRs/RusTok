#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-schema-scoped-source-event-id] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const helperPath = 'crates/rustok-index/src/application/source_event_id.rs';
requireMarkers(helperPath, [
  'pub fn derive_index_source_event_id(',
  'pub fn derive_index_schema_source_event_id(',
  'schema: &SchemaRef',
  'rustok-index-source-event-id-v1',
  'rustok-index-schema-source-event-id-v1',
  'schema.module.as_str().as_bytes()',
  'schema.entity.as_str().as_bytes()',
  'schema.version.get().to_be_bytes()',
  'IndexSourceEventIdError::ZeroSchemaVersion',
  'schema_scoped_source_event_identity_is_stable_and_schema_sensitive',
  'let current = schema(3);',
  'let replacement = schema(4);',
]);

requireMarkers('crates/rustok-index/src/application/mod.rs', [
  'derive_index_schema_source_event_id',
  'derive_index_source_event_id',
]);

const inboxPath = 'crates/rustok-index/src/migrations/m20260727_000002_create_index_delivery_state.rs';
const inbox = requireMarkers(inboxPath, [
  '.name("pk_index_inbox")',
  '.col(IndexInbox::TenantId)',
  '.col(IndexInbox::SourceName)',
  '.col(IndexInbox::DeliveryId)',
  'ColumnDef::new(IndexInbox::SchemaVersion)',
]);
const inboxPrimaryKey = inbox.slice(
  inbox.indexOf('.name("pk_index_inbox")'),
  inbox.indexOf('.foreign_key(', inbox.indexOf('.name("pk_index_inbox")')),
);
if (inboxPrimaryKey.includes('.col(IndexInbox::SchemaVersion)')) {
  fail(`${inboxPath} inbox primary key unexpectedly includes schema version; update schema-scoped delivery identity rationale`);
}

requireMarkers('crates/rustok-index/docs/m4-single-current-schema-supersession.md', [
  'Inbox delivery identity is a separate boundary',
  '`(tenant_id, source_name, delivery_id)`',
  'The legacy `derive_index_source_event_id` remains stable for existing sources',
  'must instead use `derive_index_schema_source_event_id`',
  'The selected Product source uses `derive_index_schema_source_event_id`',
]);

console.log('[verify-index-schema-scoped-source-event-id] schema supersession replay deliveries are schema-scoped without changing legacy source IDs');
