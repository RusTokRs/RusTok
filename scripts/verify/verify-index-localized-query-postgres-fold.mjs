#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-localized-query-postgres-fold] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const compilerPath = 'crates/rustok-index/src/application/postgres_localized_query.rs';
requireMarkers(compilerPath, [
  'pub struct CompiledPostgresLocalizedPageQuery',
  'pub struct LocalizedQueryPlanFingerprint',
  'pub fn compile_postgres_localized_page_query(',
  'const ROOT_ALIAS: &str = "t0";',
  'const REQUESTED_ALIAS: &str = "t1";',
  'const FALLBACK_ALIAS: &str = "t2";',
  'const ANY_LOCALE_ALIAS: &str = "t3";',
  'const EARLIER_ANCHOR_ALIAS: &str = "t4";',
  'LEFT JOIN index_entities AS {requested}',
  '{requested}.is_deleted = FALSE',
  'LEFT JOIN index_entities AS {fallback_alias}',
  '{fallback_alias}.is_deleted = FALSE',
  'NOT EXISTS (SELECT 1 FROM index_entities AS {earlier}',
  '{earlier}.locale_key < {root}.locale_key',
  '{earlier}.is_deleted = FALSE',
  'EXISTS (SELECT 1 FROM index_entities AS {any_alias}',
  '{any_alias}.is_deleted = FALSE',
  '{root}.is_deleted = FALSE',
  'CASE WHEN {requested}.entity_id IS NOT NULL THEN {requested_raw}',
  'WHEN {fallback}.entity_id IS NOT NULL THEN {fallback_raw}',
  'localized_plan_fingerprint(query, ordinary_plan_fingerprint)',
  'mode: "localized_entity_fold_plan_v1"',
  'b"rustok-index-localized-plan-v1"',
  'compile_pagination_with_lookahead',
  'COUNT(*)::bigint AS',
  '__exact_count',
  'compiled_mut(&mut self) -> &mut CompiledPostgresQuery',
]);
requireMarkers('crates/rustok-index/src/application/postgres_localized_query_result.rs', [
  'pub enum PostgresLocalizedQueryDecodeError',
  'pub fn decode_postgres_localized_query_page(',
  'LocalizedPlanFingerprintMismatch',
  'compiled.columns != expected_columns',
  'compiled.many_relations.is_empty()',
  'localized_absence_allowed: bool',
  'field.nullable || localized_absence_allowed',
  'LocalizedIndexCursor {',
  'LocalizedCursorCodec::encode_for_query(',
  '&cursor, query, self',
  'IndexQueryPage {',
]);
requireMarkers('crates/rustok-index/src/application/localized_validation.rs', [
  'LinkedPathPending(FieldPath)',
  'LocalizedProjectionMany(FieldPath)',
  'LocalizedProjectionInOrdinaryFilter(FieldPath)',
  'LocalizedProjectionInOrder(FieldPath)',
]);
requireMarkers('crates/rustok-index/src/application/mod.rs', [
  'mod postgres_localized_query;',
  'mod postgres_localized_query_result;',
  'CompiledPostgresLocalizedPageQuery, LocalizedQueryPlanFingerprint,',
  'PostgresLocalizedQueryBuildError,',
  'pub use postgres_localized_query_result::PostgresLocalizedQueryDecodeError;',
]);

const ordinaryCompiler = read('crates/rustok-index/src/application/postgres_query_sql.rs');
if (ordinaryCompiler.includes('localized_entity_fold_plan_v1')) {
  fail('ordinary exact-locale SQL compiler must not absorb localized fold semantics');
}

console.log('[verify-index-localized-query-postgres-fold] localized root identity fold page/count compiler and decoder remain source-locked');
