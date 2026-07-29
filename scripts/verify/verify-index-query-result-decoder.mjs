#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-query-result-decoder] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const decoderPath = 'crates/rustok-index/src/application/postgres_query_result.rs';
const decoder = requireMarkers(decoderPath, [
  'pub enum CompiledPostgresCell',
  'pub struct CompiledPostgresRow',
  'pub struct CompiledPostgresPageQuery',
  'pub struct IndexQueryPage',
  'pub enum PostgresQueryPageBuildError',
  'pub enum PostgresQueryDecodeError',
  'pub fn compile_postgres_page_query(',
  'apply_lookahead_bind(&mut compiled, &query.pagination)?;',
  '*value = expected_limit + 1;',
  'pub fn decode_postgres_query_page(',
  'compiled.columns != expected_columns(&plan)',
  'PlanFingerprintMismatch',
  'rows.len() > requested_page_size as usize',
  'CursorCodec::encode_for_query(&cursor, query, self)?',
  'ExactCountContractMismatch',
  'InvalidTaggedValue',
  'ManyLinkSemanticsPending',
]);

const compilePosition = decoder.indexOf('let mut compiled = self.compile_postgres_query(query)?;');
const lookaheadPosition = decoder.indexOf('apply_lookahead_bind(&mut compiled, &query.pagination)?;');
if (compilePosition < 0 || lookaheadPosition < 0 || compilePosition >= lookaheadPosition) {
  fail('validated controlled compilation must precede lookahead bind adjustment');
}

const planPosition = decoder.indexOf('let plan = self.plan_query(query)?;');
const columnPosition = decoder.indexOf('compiled.columns != expected_columns(&plan)');
const rowPosition = decoder.indexOf('let decoded = rows');
if (
  planPosition < 0 ||
  columnPosition < 0 ||
  rowPosition < 0 ||
  !(planPosition < columnPosition && columnPosition < rowPosition)
) {
  fail('plan fingerprint and column contract must be verified before row decoding');
}

for (const forbidden of [
  'rustok_product',
  'rustok_content',
  'rustok_flex',
  'DatabaseConnection',
  'Statement::from_sql',
  '.query_all(',
  '.query_one(',
  '.execute(',
  'SELECT *',
]) {
  if (decoder.includes(forbidden)) fail(`${decoderPath} contains forbidden marker ${forbidden}`);
}

requireMarkers('crates/rustok-index/src/application/postgres_query_result_tests.rs', [
  'page_compilation_adds_exactly_one_lookahead_row',
  'offset_page_compilation_preserves_offset_and_adds_lookahead',
  'decodes_projection_relations_exact_count_and_next_cursor',
  'omits_next_cursor_when_no_lookahead_row_exists',
  'rejects_page_compiled_for_different_query_semantics',
  'rejects_invalid_tagged_field_contract',
]);
requireMarkers('crates/rustok-index/src/application/mod.rs', [
  'mod postgres_query_result;',
  'mod postgres_query_result_tests;',
  'CompiledPostgresPageQuery',
  'IndexQueryPage',
  'PostgresQueryDecodeError',
]);
requireMarkers('crates/rustok-index/docs/m4-query-result-decoder.md', [
  'one-row lookahead',
  'compiled column contract',
  'query-scoped continuation cursor',
  'does not execute SQL',
  'Many-link semantics remain fail-closed',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  'M4 deterministic PostgreSQL result decoding: `complete`',
  '- [x] Add deterministic root/one-link result decoding and cursor construction.',
  '- [ ] Add explicit many-link `EXISTS` filtering and nested projection aggregation.',
]);

console.log('[verify-index-query-result-decoder] OK');
