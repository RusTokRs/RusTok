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

const planner = requireMarkers('crates/rustok-index/src/application/planner.rs', [
  'pub traverses_many: bool',
  'let mut many_paths = BTreeMap::from([(Vec::new(), false)]);',
  'link.cardinality == LinkCardinality::Many',
  'pub(crate) fn outer_joins(&self)',
  'pub struct PlannedManyProjection',
  'rustok-index-query-plan-v4',
]);
const compiler = requireMarkers('crates/rustok-index/src/application/postgres_compiler.rs', [
  'ManyLinkOrderingPending(FieldPath)',
  'ManyProjectionPlanMismatch',
  'ManyTraversalMismatch(Vec<LinkName>)',
  'expected_traverses_many',
]);
const sql = requireMarkers('crates/rustok-index/src/application/postgres_query_sql.rs', [
  'for join in plan.outer_joins()',
  'fn compile_many_exists(',
  'let mut wrappers = Vec::with_capacity(field.path.links().len());',
  'EXISTS ({wrapper}{expression})',
  '.source_version = {source_alias_q}.source_version',
  'AND NOT ({disqualifying})',
  'sql.non_null_predicate()',
  'fn compile_many_projection(',
]);

for (const [relative, source] of [
  ['crates/rustok-index/src/application/planner.rs', planner],
  ['crates/rustok-index/src/application/postgres_compiler.rs', compiler],
  ['crates/rustok-index/src/application/postgres_query_sql.rs', sql],
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

requireMarkers('crates/rustok-index/src/application/postgres_compiler_tests.rs', [
  'compiles_many_link_filter_as_correlated_exists_without_outer_join',
  'compiles_grouped_many_projection_as_row_preserving_json_aggregate',
  'assert!(!compiled.sql.contains("LEFT JOIN index_links AS \\"l1\\""))',
  'assert!(!count.sql.contains("ORDER BY"))',
]);
requireMarkers('crates/rustok-index/docs/m4-many-link-filtering.md', [
  'correlated `EXISTS` chain',
  'Independent atomic subqueries are intentional.',
  '`Ne` is deliberately not compiled as `NOT EXISTS(Eq)`',
  'does not execute SQL',
]);
requireMarkers('crates/rustok-index/docs/m4-many-link-projection.md', [
  'correlated JSONB aggregate subquery',
  'never become joins in the outer page rowset',
  'Ordering through a many link remains rejected',
  'CompiledManyRelationColumn',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  'M4 many-link `EXISTS` filtering: `complete`',
  'M4 nested many-link projection aggregation: `complete`',
  '- [x] Add explicit many-link `EXISTS` filtering.',
  '- [x] Add nested many-link projection aggregation.',
]);

console.log('[verify-index-many-link-filtering] OK');
