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
  'pub fn canonical_fallback_locale(&self)',
  'Some(*fallback) != self.requested_locale()',
  'ordinary_nodes + any_locale_nodes > MAX_LOCALIZED_FILTER_NODES',
  'pub fn any_locale_referenced_paths(&self)',
]);
requireMarkers('crates/rustok-index/src/domain/mod.rs', [
  'mod localized_query;',
  'pub use localized_query::LocalizedEntityQuery;',
]);

requireMarkers('crates/rustok-index/src/application/localized_validation.rs', [
  'pub enum LocalizedEntityQueryValidationError',
  'LocaleRequiredSchema(SchemaRef)',
  'AnyLocaleLinkedPath(FieldPath)',
  'pub fn validate_localized_entity_query(',
  'registered.schema.locale_mode != LocaleMode::Required',
  'self.validate_query(&query.query)?;',
  '.find(|path| !path.links().is_empty())',
  'probe.filter = Some(filter.clone());',
  'self.validate_query(&probe)?;',
]);

requireMarkers('crates/rustok-index/src/application/localized_cursor.rs', [
  'const LOCALIZED_SCOPED_CURSOR_VERSION: u8 = 3;',
  'pub struct LocalizedIndexCursor',
  'pub requested_locale: LocaleKey',
  'pub fallback_locale: Option<LocaleKey>',
  'pub struct LocalizedCursorCodec;',
  'mode: "localized_entity_fold_v1"',
  'fallback_locale: query.canonical_fallback_locale()',
  'any_locale_filter: &query.any_locale_filter',
  'b"rustok-index-localized-cursor-query-v1"',
  'cursor.fallback_locale.as_ref() != query.canonical_fallback_locale()',
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

requireMarkers('crates/rustok-index/src/application/mod.rs', [
  'mod localized_cursor;',
  'mod localized_validation;',
  'LocalizedCursorCodec, LocalizedCursorCodecError, LocalizedCursorValidationError,',
  'LocalizedIndexCursor,',
  'pub use localized_validation::LocalizedEntityQueryValidationError;',
]);

const port = read('crates/rustok-index/src/application/query_port.rs');
if (port.includes('execute_localized_query')) {
  fail('this contract slice must not claim PostgreSQL localized execution before the folded compiler exists');
}

requireMarkers('crates/rustok-index/docs/m7-product-storefront-localized-query-architecture.md', [
  'Status: `query_contract_source_complete_compiler_and_evidence_pending`',
  '`LocalizedEntityQuery`',
  '`LocalizedCursorCodec`',
  'wire version `3`',
  'ordinary exact-locale cursor wire version `2` remains unchanged',
]);

console.log('[verify-index-localized-query-contract] explicit fold query validation and cursor identity are source-locked; compiler/execution remain pending');
