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
  'pub fn with_localized_projection_fields(',
  'pub fn canonical_fallback_locale(&self)',
  'Some(*fallback) != self.requested_locale()',
  'pub fn is_localized_projection_path(&self, path: &FieldPath)',
  'ordinary_nodes + any_locale_nodes > MAX_LOCALIZED_FILTER_NODES',
]);
requireMarkers('crates/rustok-index/src/application/localized_validation.rs', [
  'pub enum LocalizedEntityQueryValidationError',
  'LocaleRequiredSchema(SchemaRef)',
  'LinkedPathPending(FieldPath)',
  'AnyLocaleLinkedPath(FieldPath)',
  'DuplicateLocalizedProjection(FieldPath)',
  'LocalizedProjectionInOrdinaryFilter(FieldPath)',
  'LocalizedProjectionInOrder(FieldPath)',
  'registered.schema.locale_mode != LocaleMode::Required',
  'self.validate_query(&query.query)?;',
  'probe.filter = Some(filter.clone());',
  'field.cardinality != FieldCardinality::One',
]);
requireMarkers('crates/rustok-index/src/application/localized_cursor.rs', [
  'const LOCALIZED_SCOPED_CURSOR_VERSION: u8 = 3;',
  'pub struct LocalizedIndexCursor',
  'localized_projection_fields: Vec<&\'a FieldPath>',
  'mode: "localized_entity_fold_v1"',
  'fallback_locale: query.canonical_fallback_locale()',
  'any_locale_filter: &query.any_locale_filter',
  'localized_projection_fields.sort();',
  'b"rustok-index-localized-cursor-query-v1"',
  'LocalizedCursorCodecError::UnsupportedVersion(2)',
]);

const ordinaryCursor = requireMarkers('crates/rustok-index/src/application/cursor.rs', [
  'const SCOPED_CURSOR_VERSION: u8 = 2;',
  'b"rustok-index-cursor-query-v1"',
  'pub struct IndexCursor',
]);
if (ordinaryCursor.includes('localized_entity_fold_v1')) {
  fail('ordinary cursor codec must not absorb localized fold identity');
}

requireMarkers('crates/rustok-index/src/application/query_port.rs', [
  'async fn execute_localized_query(',
  'localized Index query execution is unavailable for this adapter',
]);
requireMarkers('crates/rustok-index/src/domain/query.rs', [
  'TextLike(FieldPath, String)',
]);
requireMarkers('crates/rustok-index/docs/m7-product-storefront-localized-query-architecture.md', [
  'Status: `runtime_and_text_pattern_source_complete_adapter_and_evidence_pending`',
  '`LocalizedEntityQuery`',
  '`localized_projection_fields`',
  '`LocalizedCursorCodec`',
  'wire version `3`',
  'ordinary exact-locale cursors remain on version `2`',
  'Generic `TextLike` — source complete',
]);

console.log('[verify-index-localized-query-contract] localized query/projection/cursor contract remains source-locked with fail-closed runtime and generic text-pattern capability');