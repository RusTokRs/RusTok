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
  'pub struct CompiledPostgresPageQuery',
  'pub struct IndexNestedRelationItem',
  'pub struct IndexNestedRelationProjection',
  'pub nested_relations: Vec<IndexNestedRelationProjection>',
  'pub fn compile_postgres_page_query(',
  'apply_lookahead_bind(&mut compiled, &query.pagination)?;',
  '*value = expected_limit + 1;',
  'pub fn decode_postgres_query_page(',
  'compiled.many_relations != expected_many_relations(&plan)',
  'fn decode_nested_relation(',
  'NestedIdentityArity',
  'NestedFieldArity',
  'NilNestedIdentity',
  'DuplicateNestedIdentity',
  'decode_tagged_value(value, output_alias)?',
  'CursorCodec::encode_for_query(&cursor, query, self)?',
]);

const pageWrapper = decoder.match(
  /#\[derive\(([^)]*)\)\]\s*pub struct CompiledPostgresPageQuery/u,
);
if (!pageWrapper || pageWrapper[1].includes('Serialize') || pageWrapper[1].includes('Deserialize')) {
  fail('CompiledPostgresPageQuery must remain an opaque non-serde execution contract');
}

const planPosition = decoder.indexOf('let plan = self.plan_query(query)?;');
const metadataPosition = decoder.indexOf('compiled.many_relations != expected_many_relations(&plan)');
const rowPosition = decoder.indexOf('let decoded = rows');
if (
  planPosition < 0 || metadataPosition < 0 || rowPosition < 0
  || !(planPosition < metadataPosition && metadataPosition < rowPosition)
) {
  fail('plan and scalar/many metadata must be checked before row decoding');
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

requireMarkers('crates/rustok-index/src/application/postgres_many_projection_tests.rs', [
  'decodes_aligned_nested_identity_and_value_arrays',
  'rejects_nested_identity_and_field_arity_drift',
  'rejects_nil_and_duplicate_nested_identity_chains',
  'PostgresQueryDecodeError::NestedIdentityArity',
  'PostgresQueryDecodeError::NestedFieldArity',
  'PostgresQueryDecodeError::NilNestedIdentity',
  'PostgresQueryDecodeError::DuplicateNestedIdentity',
]);
requireMarkers('crates/rustok-index/src/application/postgres_query_result_tests.rs', [
  'page_compilation_adds_exactly_one_lookahead_row',
  'offset_page_compilation_preserves_offset_and_adds_lookahead',
  'decodes_projection_relations_exact_count_and_next_cursor',
  'omits_next_cursor_when_no_lookahead_row_exists',
  'rejects_page_compiled_for_different_query_semantics',
  'rejects_invalid_tagged_field_contract',
]);
requireMarkers('crates/rustok-index/src/application/mod.rs', [
  'mod postgres_many_projection_tests;',
  'mod query_snapshot_tests;',
  'IndexNestedRelationItem',
  'IndexNestedRelationProjection',
]);
requireMarkers('crates/rustok-index/docs/m4-query-snapshots.md', [
  'identity arity drift',
  'selected-field arity drift',
  'nil nested identities',
  'duplicate complete identity chains',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  'M4 deterministic PostgreSQL result decoding: `complete`',
  '- [x] Add nested many-link projection aggregation.',
  '- [x] Add retained v4 plan/SQL snapshots and synchronized source guards.',
  '- [ ] Execute PostgreSQL/reference-engine equivalence capture and admit retained live evidence.',
]);

console.log('[verify-index-query-result-decoder] OK');