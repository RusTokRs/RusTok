#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-many-link-aggregate-ordering] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};
const forbidMarkers = (relative, source, markers) => {
  for (const marker of markers) {
    if (source.includes(marker)) fail(`${relative} contains forbidden marker ${marker}`);
  }
};

const queryPath = 'crates/rustok-index/src/domain/query.rs';
requireMarkers(queryPath, [
  'pub enum ManyOrderAggregate',
  'Min,',
  'Max,',
  'MinAsc,',
  'MinDesc,',
  'MaxAsc,',
  'MaxDesc,',
  'pub const fn aggregate(self)',
  'pub const fn base_direction(self)',
  'explicit_many_order_modes_expose_aggregate_and_base_direction',
]);
requireMarkers('crates/rustok-index/src/domain/mod.rs', ['ManyOrderAggregate']);

const validationPath = 'crates/rustok-index/src/application/aggregate_ordering.rs';
const validation = requireMarkers(validationPath, [
  'pub enum AggregateOrderValidationError',
  'AggregateRequiresManyLink',
  'AggregateRequiresOrderedScalar',
  'AggregateRequiresOffsetPagination',
  'pub fn validate_query_with_aggregate_ordering',
  'order.direction.aggregate().is_some()',
  'matches!(&query.pagination, Pagination::Offset { .. })',
  'self.validate_query(&ordinary)?;',
  'resolved.traverses_many',
  'IndexValueType::Integer | IndexValueType::String | IndexValueType::Timestamp',
  'for unsupported in [IndexValueType::Decimal, IndexValueType::Uuid]',
  'aggregate_cursor_pagination_remains_rejected',
]);
forbidMarkers(validationPath, validation, [
  'IndexValueType::Integer\n            | IndexValueType::Decimal',
  'unwrap_or(',
  'first()',
  'ordinal',
]);

const plannerPath = 'crates/rustok-index/src/application/planner.rs';
requireMarkers(plannerPath, [
  'self.validate_query_with_aggregate_ordering(query)',
  'QueryPlanError::Validation(error)',
  'QueryPlanError::Registry(error)',
  'QueryPlanError::AggregateValidation(error)',
  'if order.direction.aggregate().is_some()',
  'field.nullable = true;',
  'rustok-index-query-plan-v4',
]);

const compilerPath = 'crates/rustok-index/src/application/postgres_compiler.rs';
const compiler = requireMarkers(compilerPath, [
  'AggregateOrderingWithoutManyLink',
  'AggregateOrderingUnsupportedType',
  'AggregateOrderingRequiresOffsetPagination',
  'ManyLinkOrderingPending',
  'expected.nullable = true;',
  'aggregate_type_supported(order.field.value_type)',
  'has_aggregate_order && !matches!(&self.pagination, Pagination::Offset { .. })',
  'IndexValueType::Integer | IndexValueType::String | IndexValueType::Timestamp',
]);
forbidMarkers(compilerPath, compiler, [
  'IndexValueType::Integer\n            | IndexValueType::Decimal',
]);

const sqlPath = 'crates/rustok-index/src/application/postgres_query_sql.rs';
const sql = requireMarkers(sqlPath, [
  'fn compile_many_order_aggregate(',
  'ManyOrderAggregate::Min => "MIN"',
  'ManyOrderAggregate::Max => "MAX"',
  'FROM index_links AS {link_alias}',
  "jsonb_build_object('type', '{}', 'value', to_jsonb({scalar}))",
  'order.direction.base_direction()',
  'ASC NULLS LAST',
  'DESC NULLS FIRST',
  'entity_id ASC',
]);
forbidMarkers(sqlPath, sql, [
  'array_agg(',
  'ordinal LIMIT 1',
  'target_entity_id LIMIT 1',
]);

requireMarkers('crates/rustok-index/src/application/aggregate_ordering_tests.rs', [
  'min_asc_compiles_correlated_tagged_order_value',
  'max_desc_compiles_explicit_null_policy',
  'aggregate_cursor_decimal_and_uuid_modes_fail_closed',
  'forged_plans_remain_fail_closed',
  'IndexValueType::String',
  "jsonb_build_object('type', 'string'",
  'AggregateOrderingRequiresOffsetPagination',
  'assert!(!compiled',
  '.contains(" LEFT JOIN index_links AS \\"l1\\""));',
]);
for (const referencePath of [
  'crates/rustok-index/src/application/reference.rs',
  'crates/rustok-index/src/infrastructure/postgres/postgres_reference_equivalence_tests/reference_fixture.rs',
]) {
  requireMarkers(referencePath, [
    'match direction.base_direction()',
    'base_direction returns a physical direction',
  ]);
}

const contractPath = 'crates/rustok-index/contracts/m4-many-link-aggregate-ordering.json';
const contract = JSON.parse(read(contractPath));
if (contract.schema_version !== 1 || contract.owner !== 'rustok-index') {
  fail(`${contractPath} identity drifted`);
}
if (contract.status !== 'source_complete_execution_pending') {
  fail(`${contractPath} must remain execution pending`);
}
if (JSON.stringify(contract.query_contract?.many_link_modes)
  !== JSON.stringify(['min_asc', 'min_desc', 'max_asc', 'max_desc'])) {
  fail(`${contractPath} aggregate modes drifted`);
}
if (contract.query_contract?.plain_many_link_asc_desc_rejected !== true
  || contract.query_contract?.aggregate_requires_many_link !== true
  || contract.query_contract?.aggregate_cursor_supported !== false
  || contract.query_contract?.pagination !== 'bounded_offset') {
  fail(`${contractPath} query boundary drifted`);
}
if (JSON.stringify(contract.supported_terminal_types)
  !== JSON.stringify(['integer', 'string', 'timestamp'])) {
  fail(`${contractPath} supported type contract drifted`);
}
if (!contract.rejected_terminal_types?.includes('decimal')
  || !contract.remaining?.includes('exact decimal tagged-order wire contract')) {
  fail(`${contractPath} must keep decimal fail-closed pending an exact wire contract`);
}
if (contract.postgresql?.strategy !== 'correlated_scalar_subquery'
  || contract.postgresql?.outer_many_join !== false
  || contract.postgresql?.caller_sql !== false
  || contract.postgresql?.implicit_first_row !== false
  || contract.postgresql?.implicit_link_ordinal !== false) {
  fail(`${contractPath} PostgreSQL boundary drifted`);
}
for (const key of ['cargo_run', 'tests_run', 'postgresql_run', 'node_verifiers_run', 'workflows_run', 'ci_run']) {
  if (contract.validation?.[key] !== false) fail(`${contractPath} must not claim ${key}`);
}

requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-many-link-aggregate-ordering.mjs'",
]);
requireMarkers('crates/rustok-index/docs/m4-many-link-aggregate-ordering.md', [
  'Status: `source_complete_execution_pending`',
  '`crates/rustok-index/contracts/m4-many-link-aggregate-ordering.json`',
  '`min_asc`',
  '`max_desc`',
  'bounded offset',
  'Decimal and UUID',
  'exact tagged-wire contract',
  'Aggregate cursor continuation remains open',
  'Not run by the implementation agent',
]);

console.log('[verify-index-many-link-aggregate-ordering] OK');
