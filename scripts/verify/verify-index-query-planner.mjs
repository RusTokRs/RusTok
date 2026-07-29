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
  'pub struct QueryPlanFingerprint',
  'pub fn plan_query(&self, query: &IndexQuery)',
  'self.validate_query(query)?;',
  'collect_link_prefixes(query)',
  'BTreeMap::from([(Vec::new(), ROOT_ALIAS.to_owned())])',
  'format!("t{}", index + 1)',
  'postcard::to_stdvec(self)?',
  'rustok-index-query-plan-v1',
]);

const validation = planner.indexOf('self.validate_query(query)?;');
const aliasing = planner.indexOf('collect_link_prefixes(query)');
if (validation < 0 || aliasing < 0 || validation > aliasing) {
  fail('query validation must run before link-path planning');
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
  'validation_precedes_plan_construction',
  'fingerprint_changes_with_order_semantics',
  'assert_eq!(first.root_alias, "t0")',
  'assert_eq!(first.joins[0].alias, "t1")',
]);

requireMarkers('crates/rustok-index/src/application/mod.rs', [
  'mod planner;',
  'mod planner_tests;',
  'ExecutableQueryPlan',
  'QueryPlanFingerprint',
]);

requireMarkers('crates/rustok-index/docs/m4-query-planner.md', [
  'M4 deterministic executable query planning: `source_complete_execution_pending`',
  'M3 production partition lifecycle: `blocked_by_retained_packet`',
  'M4 controlled PostgreSQL query compilation: `source_complete_execution_pending`',
  'Typed filter/order/count/keyset compilation remains the next bounded M4 slice.',
  'Not run by the implementation agent',
]);

requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '### M4 - Query engine v1',
  '- [x] Compile plans through SeaQuery or controlled SQL.',
  'Partition cutover remains forbidden until one retained real',
]);

console.log('[verify-index-query-planner] OK');
