#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-query-planner] ${message}`);
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
  'pub struct ExecutableQueryPlan',
  'pub struct PlannedJoin',
  'pub struct PlannedField',
  'pub struct PlannedManyProjection',
  'pub many_projections: Vec<PlannedManyProjection>',
  'pub traverses_many: bool',
  'pub fn plan_query(&self, query: &IndexQuery)',
  'self.validate_query_with_aggregate_ordering(query)',
  'collect_link_prefixes(query)',
  'let mut many_paths = BTreeMap::from([(Vec::new(), false)]);',
  'let many_projections = derive_many_projections(&projection);',
  'pub(crate) fn derive_many_projections(',
  'pub(crate) fn outer_projection(&self)',
  'rustok-index-query-plan-v4',
]);

const aggregateOrderingPath = 'crates/rustok-index/src/application/aggregate_ordering.rs';
requireMarkers(aggregateOrderingPath, [
  'pub fn validate_query_with_aggregate_ordering(',
  'query.validate_shape().map_err(QueryValidationError::from)?;',
  'self.validate_query(&ordinary)?;',
]);

const validation = planner.indexOf('self.validate_query_with_aggregate_ordering(query)');
const aliasing = planner.indexOf('collect_link_prefixes(query)');
const fields = planner.indexOf('let referenced_paths = query');
const grouping = planner.indexOf('let many_projections = derive_many_projections(&projection);');
if (
  validation < 0 || aliasing < 0 || fields < 0 || grouping < 0
  || !(validation < aliasing && aliasing < fields && fields < grouping)
) {
  fail('validation, aliases, typed fields, and many projection grouping must remain ordered');
}

for (const forbidden of [
  'rustok_product',
  'rustok_content',
  'rustok_forum',
  'SELECT ',
  'INSERT ',
  'UPDATE ',
  'DELETE ',
  'sea_orm::',
]) {
  if (planner.includes(forbidden)) fail(`${plannerPath} contains forbidden marker ${forbidden}`);
}

requireMarkers('crates/rustok-index/src/application/planner_tests.rs', [
  'aliases_do_not_depend_on_reference_encounter_order',
  'many_traversal_propagates_through_descendant_joins_and_fields',
  'validation_precedes_plan_construction',
  'fingerprint_changes_with_order_semantics',
]);
requireMarkers('crates/rustok-index/src/application/query_snapshot_tests.rs', [
  'retained_v4_plan_and_sql_snapshots_are_stable',
  'render_plan(&plan)',
  'PLAN_SNAPSHOT',
  'many:{}|identities={}|fields={}',
]);
requireMarkers('crates/rustok-index/src/application/snapshots/m4_many_projection.plan.snap', [
  'root=rustok-product::product@1',
  'join:variants|t0->t1|rustok-product::variant@1|many|traverses_many=true',
  'many:variants|identities=variants|fields=variants.id',
]);
requireMarkers('crates/rustok-index/docs/m4-query-snapshots.md', [
  'Status: `source_complete_owner_execution_pending`',
  'executable-plan v4',
  'does not claim PostgreSQL/reference-engine',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '### M4 - Query engine v1',
  '- [x] Add nested many-link projection aggregation.',
  '- [x] Add retained v4 plan/SQL snapshots and synchronized source guards.',
  '- [ ] Execute PostgreSQL/reference-engine equivalence capture and admit retained live evidence.',
  'Partition cutover remains forbidden until one retained real',
]);

console.log('[verify-index-query-planner] OK');
