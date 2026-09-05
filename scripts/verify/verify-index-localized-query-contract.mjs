#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-localized-query-contract] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

requireMarkers('crates/rustok-index/src/domain/localized_query.rs', [
  'pub struct LocalizedEntityQuery',
  'pub query: IndexQuery',
  'pub fallback_locale: Option<LocaleKey>',
  'pub any_locale_filter: Option<FilterExpr>',
  'pub localized_projection_fields: Vec<FieldPath>',
  'pub identity_order_direction: OrderDirection',
  'pub fn with_localized_projection_fields(',
  'pub fn with_identity_order_direction(',
  'pub fn canonical_fallback_locale(&self)',
  'ordinary_nodes + any_locale_nodes > MAX_LOCALIZED_FILTER_NODES',
]);
requireMarkers('crates/rustok-index/src/application/localized_validation.rs', [
  'LocaleRequiredSchema(SchemaRef)',
  'InvalidIdentityOrderDirection',
  'OrderDirection::Asc | OrderDirection::Desc',
  'LinkedPathPending(FieldPath)',
  'AnyLocaleLinkedPath(FieldPath)',
  'LocalizedProjectionInOrdinaryFilter(FieldPath)',
  'LocalizedProjectionInOrder(FieldPath)',
  'self.validate_query(&query.query)?;',
]);
requireMarkers('crates/rustok-index/src/application/localized_cursor.rs', [
  'const LOCALIZED_SCOPED_CURSOR_VERSION: u8 = 3;',
  'identity_order_direction: OrderDirection',
  'identity_order_direction: query.identity_order_direction',
  'mode: "localized_entity_fold_v1"',
  'localized_projection_fields.sort();',
]);

const ordinaryCursor = requireMarkers('crates/rustok-index/src/application/cursor.rs', [
  'const SCOPED_CURSOR_VERSION: u8 = 2;',
  'b"rustok-index-cursor-query-v1"',
]);
if (ordinaryCursor.includes('identity_order_direction')) {
  fail('ordinary cursor codec must not absorb localized identity ordering');
}

requireMarkers('crates/rustok-index/src/application/query_port.rs', [
  'async fn execute_localized_query(',
  'localized Index query execution is unavailable for this adapter',
]);
requireMarkers('crates/rustok-index/src/domain/query.rs', ['TextLike(FieldPath, String)']);
requireMarkers('crates/rustok-index/docs/m7-product-storefront-localized-query-architecture.md', [
  'Status: `runtime_text_pattern_identity_order_source_complete_adapter_and_evidence_pending`',
  '`LocalizedEntityQuery`',
  '`identity_order_direction`',
  '`LocalizedCursorCodec` remains on localized wire version `3`',
  'generic `TextLike`',
]);

console.log('[verify-index-localized-query-contract] localized query/projection/cursor/identity-order contract is source-locked');