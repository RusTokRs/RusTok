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
  'pub value_type: IndexValueType',
  'pub cardinality: FieldCardinality',
  'pub nullable: bool',
  'pub traverses_many: bool',
  'pub referenced_fields: BTreeMap<FieldPath, PlannedField>',
  'pub fn plan_query(&self, query: &IndexQuery)',
  'self.validate_query(query)?;',
  'collect_link_prefixes(query)',
  'collect::<BTreeSet<_>>()',
  'BTreeMap::from([(Vec::new(), ROOT_ALIAS.to_owned())])',
  'let mut many_paths = BTreeMap::from([(Vec::new(), false)]);',
  'link.cardinality == LinkCardinality::Many',
  'many_paths.insert(path.clone(), traverses_many);',
  'pub(crate) fn outer_joins(&self)',
  'format!("t{}", index + 1)',
  'postcard::to_stdvec(self)?',
  'rustok-index-query-plan-v3',
]);

const validation = planner.indexOf('self.validate_query(query)?;');
const aliasing = planner.indexOf('collect_link_prefixes(query)');
const manyPlanning = planner.indexOf('let mut many_paths = BTreeMap::from([(Vec::new(), false)]);');
const fieldContracts = planner.indexOf('let referenced_paths = query');
if (
  validation < 0 ||
  aliasing < 0 ||
  manyPlanning < 0 ||
  fieldContracts < 0 ||
  !(validation < aliasing && aliasing < manyPlanning && manyPlanning < fieldContracts)
) {
  fail('query validation, aliases, and many-path propagation must precede field contracts');
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
  'assert_eq!(first.referenced_fields, second.referenced_fields)',
  'assert_eq!(first.order_by[0].field.value_type, IndexValueType::Uuid)',
  'assert!(plan.joins.iter().all(|join| join.traverses_many))',
  'assert_eq!(plan.outer_joins().count(), 0)',
]);

requireMarkers('crates/rustok-index/src/application/mod.rs', [
  'mod planner;',
  'mod planner_tests;',
  'ExecutableQueryPlan',
  'QueryPlanFingerprint',
]);

requireMarkers('crates/rustok-index/docs/m4-query-planner.md', [
  'M4 typed referenced-field contracts: `source_complete_execution_pending`',
  'rustok-index-query-plan-v3',
  'M4 many-link `EXISTS` filtering: `source_complete_execution_pending`',
  'Many-cardinality projection remains fail-closed with',
  'Not run by the implementation agent',
]);

requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '### M4 - Query engine v1',
  '- [x] Compile plans through SeaQuery or controlled SQL.',
  '- [x] Add explicit many-link `EXISTS` filtering.',
  '- [ ] Add nested many-link projection aggregation.',
  'Partition cutover remains forbidden until one retained real',
]);

console.log('[verify-index-query-planner] OK');
