#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-many-link-filtering] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const plannerPath = 'crates/rustok-index/src/application/planner.rs';
const planner = requireMarkers(plannerPath, [
  'pub traverses_many: bool',
  'let mut many_paths = BTreeMap::from([(Vec::new(), false)]);',
  'link.cardinality == LinkCardinality::Many',
  'many_paths.insert(path.clone(), traverses_many);',
  'pub(crate) fn outer_joins(&self)',
  'rustok-index-query-plan-v3',
]);

const compilerPath = 'crates/rustok-index/src/application/postgres_compiler.rs';
const compiler = requireMarkers(compilerPath, [
  'ManyLinkProjectionPending(FieldPath)',
  'ManyLinkOrderingPending(FieldPath)',
  'MissingJoinPlan(Vec<LinkName>)',
  'ManyTraversalMismatch(Vec<LinkName>)',
  'expected_traverses_many',
  'if field.traverses_many',
  'if order.field.traverses_many',
]);

const sqlPath = 'crates/rustok-index/src/application/postgres_query_sql.rs';
const sql = requireMarkers(sqlPath, [
  'for join in plan.outer_joins()',
  'for (index, join) in plan.outer_joins().enumerate()',
  'fn compile_many_exists(',
  'let mut wrappers = Vec::with_capacity(field.path.links().len());',
  'EXISTS ({wrapper}{expression})',
  '.source_version = {source_alias_q}.source_version',
  'AND NOT ({disqualifying})',
  'sql.non_null_predicate()',
  'compile_many_exists(plan, field, bindings',
]);

const outerJoinPosition = sql.indexOf('for (index, join) in plan.outer_joins().enumerate()');
const filterPosition = sql.indexOf('fn compile_filter(');
const manyPosition = sql.indexOf('fn compile_many_exists(');
const countPosition = sql.indexOf('fn compile_exact_count(');
if (
  outerJoinPosition < 0 ||
  filterPosition < 0 ||
  manyPosition < 0 ||
  countPosition < 0 ||
  !(outerJoinPosition < filterPosition && filterPosition < manyPosition && manyPosition < countPosition)
) {
  fail('outer joins, filter dispatch, correlated many predicates, and count must keep deterministic layering');
}

for (const [relative, source] of [
  [plannerPath, planner],
  [compilerPath, compiler],
  [sqlPath, sql],
]) {
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
    if (source.includes(forbidden)) fail(`${relative} contains forbidden marker ${forbidden}`);
  }
}

requireMarkers('crates/rustok-index/src/application/planner_tests.rs', [
  'many_traversal_propagates_through_descendant_joins_and_fields',
  'assert!(plan.joins.iter().all(|join| join.traverses_many))',
  'assert_eq!(plan.outer_joins().count(), 0)',
]);
requireMarkers('crates/rustok-index/src/application/postgres_compiler_tests.rs', [
  'compiles_nested_many_link_filter_as_correlated_exists_without_outer_join',
  'compiles_many_link_ne_and_is_null_with_reference_totality',
  'rejects_many_link_projection_until_nested_aggregation_exists',
  'rejects_tampered_many_traversal_metadata',
  'assert!(!compiled.sql.contains("LEFT JOIN index_links AS \\"l1\\""))',
  'assert!(!count.sql.contains("ORDER BY"))',
  'assert!(!count.sql.contains("LIMIT"))',
]);
requireMarkers('crates/rustok-index/src/application/postgres_query_result.rs', [
  'plan.outer_joins().map(|join| CompiledQueryColumn::EntityId',
]);
requireMarkers('crates/rustok-index/docs/m4-many-link-filtering.md', [
  'nested correlated `EXISTS` chain',
  'Independent atomic subqueries are intentional.',
  '`Ne` is deliberately not compiled as `NOT EXISTS(Eq)`',
  'ManyLinkProjectionPending',
  'does not execute SQL',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  'M4 many-link `EXISTS` filtering: `complete`',
  '- [x] Add explicit many-link `EXISTS` filtering.',
  '- [ ] Add nested many-link projection aggregation.',
  'ManyLinkProjectionPending',
]);

console.log('[verify-index-many-link-filtering] OK');
