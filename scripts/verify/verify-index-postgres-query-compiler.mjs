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
  'pub struct CompiledManyRelationColumn',
  'pub many_relations: Vec<CompiledManyRelationColumn>',
  'pub enum PostgresQueryCompileError',
  'ManyLinkOrderingPending(FieldPath)',
  'ManyProjectionPlanMismatch',
  'validate_many_projection_contract(self)?;',
  'derive_many_projections(&plan.projection)',
  'self.validate_compiler_contract(cursor)?;',
  'super::postgres_query_sql::compile_postgres_plan(self, cursor)',
]);

const validation = compiler.indexOf('self.validate_compiler_contract(cursor)?;');
const emission = compiler.indexOf('super::postgres_query_sql::compile_postgres_plan(self, cursor)');
if (validation < 0 || emission < 0 || validation >= emission) {
  fail('plan and cursor invariants must be checked before SQL emission');
}

const sqlPath = 'crates/rustok-index/src/application/postgres_query_sql.rs';
const sql = requireMarkers(sqlPath, [
  'pub(super) fn compile_postgres_plan(',
  'let mut many_relations = Vec::new();',
  'for join in plan.outer_joins()',
  'if field.traverses_many {',
  'for (index, projection) in plan.many_projections.iter().enumerate()',
  'let aggregate = compile_many_projection(plan, projection, index, &mut bindings)?;',
  'CompiledManyRelationColumn',
  'fn compile_many_projection(',
  "jsonb_build_object('entity_ids'",
  "'values'",
  'jsonb_agg({item} ORDER BY {})',
  '.ordinal ASC',
  'fn compile_many_exists(',
  'EXISTS ({wrapper}{expression})',
  'compile_keyset(plan, cursor, &mut bindings)?',
  'ASC NULLS LAST',
  'DESC NULLS FIRST',
  'SELECT COUNT(*)::bigint AS \\"__exact_count\\"',
  'format!("${}", self.values.len())',
]);

const basePosition = sql.indexOf('let base = compile_base(plan, &mut bindings);');
const projectionPosition = sql.indexOf('for (index, projection) in plan.many_projections.iter().enumerate()');
const filterPosition = sql.indexOf('predicates.push(compile_filter(plan, filter, &mut bindings)?);');
const paginationPosition = sql.indexOf('let pagination = compile_pagination(&plan.pagination, &mut bindings)?;');
if (
  basePosition < 0 || projectionPosition < 0 || filterPosition < 0 || paginationPosition < 0
  || !(basePosition < projectionPosition && projectionPosition < filterPosition && filterPosition < paginationPosition)
) {
  fail('scope, projections, filters, and pagination must retain deterministic bind order');
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

requireMarkers('crates/rustok-index/src/application/postgres_compiler_tests.rs', [
  'compiles_root_projection_with_bound_scope_and_limit',
  'compiles_one_link_projection_without_interpolating_contract_values',
  'compiles_typed_filters_order_exact_count_and_bounded_offset',
  'compiles_validated_lexicographic_keyset_with_entity_tie_breaker',
  'rejects_cursor_reuse_across_query_semantics_before_sql_compilation',
  'compiles_many_link_filter_as_correlated_exists_without_outer_join',
  'compiles_grouped_many_projection_as_row_preserving_json_aggregate',
  'rejects_tampered_many_projection_plan',
  'PostgresQueryCompileError::ManyProjectionPlanMismatch',
]);
requireMarkers('crates/rustok-index/src/application/query_snapshot_tests.rs', [
  'SQL_SNAPSHOT',
  'COMPILED_SNAPSHOT',
  'format!("{}\\n", compiled.sql)',
  'render_compiled(&compiled.binds, &compiled.columns, &compiled.many_relations)',
]);
requireMarkers('crates/rustok-index/src/application/snapshots/m4_many_projection.sql', [
  'AS "__many_0"',
  'jsonb_agg(',
  'ORDER BY "mp0_l1".ordinal ASC',
  'LIMIT $12',
]);
requireMarkers('crates/rustok-index/docs/m4-query-snapshots.md', [
  'complete controlled PostgreSQL statement',
  'ordered bind DTOs',
  'byte-for-byte',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  'M4 controlled PostgreSQL query compilation: `complete`',
  'M4 nested many-link projection aggregation: `complete`',
  '- [x] Add retained v4 plan/SQL snapshots and synchronized source guards.',
  '- [ ] Execute PostgreSQL/reference-engine equivalence capture and admit retained live evidence.',
]);

console.log('[verify-index-postgres-query-compiler] OK');
