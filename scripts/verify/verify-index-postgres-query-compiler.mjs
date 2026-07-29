#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-postgres-query-compiler] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const compilerPath = 'crates/rustok-index/src/application/postgres_compiler.rs';
const compiler = requireMarkers(compilerPath, [
  'pub enum PostgresBindValue',
  'Boolean(bool)',
  'Decimal(Decimal)',
  'Timestamp(DateTime<Utc>)',
  'Json(JsonValue)',
  'pub struct CompiledPostgresCount',
  'pub struct CompiledPostgresQuery',
  'pub enum PostgresQueryBuildError',
  'pub enum PostgresQueryCompileError',
  'pub fn compile_postgres_query(',
  'CursorCodec::decode_scoped_for_query(',
  'pub fn compile_postgres(&self)',
  'self.validate_compiler_contract(cursor)?;',
  'super::postgres_query_sql::compile_postgres_plan(self, cursor)',
  'ManyLinkSemanticsPending',
  'self.referenced_fields.get(&field.path) != Some(field)',
  'self.path_aliases.get(&Vec::new())',
  'validate_alias(&self.root_alias)?;',
]);

const validation = compiler.indexOf('self.validate_compiler_contract(cursor)?;');
const emission = compiler.indexOf('super::postgres_query_sql::compile_postgres_plan(self, cursor)');
if (validation < 0 || emission < 0 || validation >= emission) {
  fail('plan and cursor invariants must be checked before SQL emission');
}

const sqlPath = 'crates/rustok-index/src/application/postgres_query_sql.rs';
const sql = requireMarkers(sqlPath, [
  'pub(super) fn compile_postgres_plan(',
  'let base = compile_base(plan, &mut bindings);',
  'FilterExpr::And(children)',
  'FilterExpr::Contains(path, value)',
  'FilterExpr::IsNull(path, expected_null)',
  'COALESCE(',
  '::boolean',
  '::bigint',
  '::numeric',
  'COLLATE \\"C\\"',
  '::uuid',
  '::timestamptz',
  'compile_keyset(plan, cursor, &mut bindings)?',
  'cursor_field_predicates(',
  '.entity_id > {entity_id}',
  'ASC NULLS LAST',
  'DESC NULLS FIRST',
  'LIMIT {limit} OFFSET {offset}',
  'SELECT COUNT(*)::bigint AS \\"__exact_count\\"',
  'PostgresBindValue::Json(encoded)',
  'format!("${}", self.values.len())',
]);

const basePosition = sql.indexOf('let base = compile_base(plan, &mut bindings);');
const filterPosition = sql.indexOf('predicates.push(compile_filter(plan, filter, &mut bindings)?);');
const keysetPosition = sql.indexOf('predicates.push(compile_keyset(plan, cursor, &mut bindings)?);');
const orderPosition = sql.indexOf('let order = compile_order(plan, &mut bindings);');
const paginationPosition = sql.indexOf('let pagination = compile_pagination(&plan.pagination, &mut bindings)?;');
if (
  basePosition < 0 ||
  filterPosition < 0 ||
  keysetPosition < 0 ||
  orderPosition < 0 ||
  paginationPosition < 0 ||
  !(basePosition < filterPosition &&
    filterPosition < keysetPosition &&
    keysetPosition < orderPosition &&
    orderPosition < paginationPosition)
) {
  fail('scope, filters, keyset, ordering, and pagination must retain deterministic emission order');
}

for (const [relative, source] of [[compilerPath, compiler], [sqlPath, sql]]) {
  for (const forbidden of [
    'rustok_product',
    'rustok_content',
    'rustok_flex',
    'SELECT *',
    'query_one(',
    'query_all(',
    'execute(',
    'DatabaseConnection',
    'Statement::from_sql',
  ]) {
    if (source.includes(forbidden)) fail(`${relative} contains forbidden marker ${forbidden}`);
  }
}

requireMarkers('crates/rustok-index/src/application/cursor.rs', [
  'const SCOPED_CURSOR_VERSION: u8 = 2;',
  'pub fn encode_for_query(',
  'pub fn decode_scoped_for_query(',
  'rustok-index-cursor-query-v1',
  'QueryFingerprintMismatch',
  'OrderValueTypeMismatch',
  'resolve_order_value_type(',
]);
requireMarkers('crates/rustok-index/src/application/postgres_compiler_tests.rs', [
  'compiles_root_projection_with_bound_scope_and_limit',
  'compiles_one_link_projection_without_interpolating_contract_values',
  'compiles_typed_filters_order_exact_count_and_bounded_offset',
  'compiles_validated_lexicographic_keyset_with_entity_tie_breaker',
  'rejects_cursor_reuse_across_query_semantics_before_sql_compilation',
  'rejects_many_link_semantics_before_sql_is_emitted',
  'rejects_tampered_path_alias_mapping',
  'assert!(!compiled.sql.contains(&tenant_id.to_string()))',
  'assert!(!compiled.sql.contains("sales_channel"))',
]);
requireMarkers('crates/rustok-index/src/application/mod.rs', [
  'mod postgres_compiler;',
  'mod postgres_query_sql;',
  'mod postgres_compiler_tests;',
  'CompiledPostgresCount',
  'CompiledPostgresQuery',
  'PostgresQueryBuildError',
]);
requireMarkers('crates/rustok-index/docs/m4-postgres-query-compiler.md', [
  '## Supported query semantics',
  'Atomic predicates are compiled into total booleans.',
  'Ascending order uses `NULLS LAST`; descending order uses `NULLS FIRST`.',
  'CompiledPostgresCount',
  'ManyLinkSemanticsPending',
  'does not connect to PostgreSQL',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  'M4 controlled PostgreSQL query compilation: `complete`',
  'M4 typed root/one-link query semantics: `complete`',
  '- [x] Compile plans through SeaQuery or controlled SQL.',
  '- [x] Keep offset pagination bounded and compatibility-only.',
  'Many-cardinality paths remain fail-closed with `ManyLinkSemanticsPending`',
]);

console.log('[verify-index-postgres-query-compiler] OK');
