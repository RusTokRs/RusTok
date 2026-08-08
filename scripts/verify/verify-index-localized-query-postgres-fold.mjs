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
const compiler = requireMarkers(compilerPath, [
  'pub struct CompiledPostgresLocalizedPageQuery',
  'pub struct LocalizedQueryPlanFingerprint',
  'pub fn compile_postgres_localized_page_query(',
  'const ROOT_ALIAS: &str = "t0";',
  'const REQUESTED_ALIAS: &str = "t1";',
  'const FALLBACK_ALIAS: &str = "t2";',
  'const ANY_LOCALE_ALIAS: &str = "t3";',
  'const EARLIER_ANCHOR_ALIAS: &str = "t4";',
  'LEFT JOIN index_entities AS {requested}',
  'LEFT JOIN index_entities AS {fallback_alias}',
  'NOT EXISTS (SELECT 1 FROM index_entities AS {earlier}',
  '{earlier}.locale_key < {root}.locale_key',
  'EXISTS (SELECT 1 FROM index_entities AS {any_alias}',
  'CASE WHEN {requested}.entity_id IS NOT NULL THEN {requested_raw}',
  'WHEN {fallback}.entity_id IS NOT NULL THEN {fallback_raw}',
  'localized_plan_fingerprint(query, ordinary_plan_fingerprint)',
  'mode: "localized_entity_fold_plan_v1"',
  'b"rustok-index-localized-plan-v1"',
  'compile_pagination_with_lookahead',
  'SELECT COUNT(*)::bigint AS "__exact_count"',
  'compiled_mut(&mut self) -> &mut CompiledPostgresQuery',
]);
for (const alias of ['t0', 't1', 't2', 't3', 't4']) {
  if (!compiler.includes(`is_deleted = FALSE`)) {
    fail(`${compilerPath} does not retain canonical is_deleted anchors`);
  }
}

requireMarkers('crates/rustok-index/src/application/postgres_localized_query_result.rs', [
  'pub enum PostgresLocalizedQueryDecodeError',
  'pub fn decode_postgres_localized_query_page(',
  'LocalizedPlanFingerprintMismatch',
  'compiled.columns != expected_columns',
  'compiled.many_relations.is_empty()',
  'localized_absence_allowed: bool',
  'field.nullable || localized_absence_allowed',
  'LocalizedIndexCursor {',
  'LocalizedCursorCodec::encode_for_query(&cursor, query, self)',
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

const port = read('crates/rustok-index/src/application/query_port.rs');
if (port.includes('execute_localized_query')) {
  fail('compiler/decoder slice must not publish runtime localized execution yet');
}

console.log('[verify-index-localized-query-postgres-fold] localized root identity fold page/count compiler and decoder are source-locked; runtime admission remains pending');
