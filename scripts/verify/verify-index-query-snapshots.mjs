#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-query-snapshots] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};
const normalizeWhitespace = (value) => value.replace(/\s+/gu, ' ').trim();
const requireNormalizedMarkers = (relative, markers) => {
  const source = normalizeWhitespace(read(relative));
  for (const marker of markers) {
    if (!source.includes(normalizeWhitespace(marker))) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const testPath = 'crates/rustok-index/src/application/query_snapshot_tests.rs';
const test = requireMarkers(testPath, [
  'include_str!("snapshots/m4_many_projection.plan.snap")',
  'include_str!("snapshots/m4_many_projection.sql")',
  'include_str!("snapshots/m4_many_projection.compiled.snap")',
  'Uuid::from_u128(1)',
  'retained_v4_plan_and_sql_snapshots_are_stable',
  'assert_eq!(render_plan(&plan), PLAN_SNAPSHOT);',
  'assert_eq!(format!("{}\\n", compiled.sql), SQL_SNAPSHOT);',
  'render_compiled(&compiled.binds, &compiled.columns, &compiled.many_relations)',
]);

for (const forbidden of [
  'UPDATE_SNAPSHOTS',
  'INSTA_UPDATE',
  'fs::write',
  'std::fs::write',
  'File::create',
  'OpenOptions',
  'new_v4()',
]) {
  if (test.includes(forbidden)) fail(`${testPath} contains forbidden marker ${forbidden}`);
}

const planPath = 'crates/rustok-index/src/application/snapshots/m4_many_projection.plan.snap';
const plan = requireMarkers(planPath, [
  'root=rustok-product::product@1',
  'alias:<root>=t0',
  'alias:variants=t1',
  'traverses_many=true',
  'many:variants|identities=variants|fields=variants.id',
  'pagination=cursor:first=2:after=false',
]);
if (!plan.endsWith('\n')) fail(`${planPath} must end with one newline`);

const sqlPath = 'crates/rustok-index/src/application/snapshots/m4_many_projection.sql';
const sql = requireMarkers(sqlPath, [
  'SELECT "t0".entity_id AS "__t0_entity_id"',
  'AS "__many_0"',
  'jsonb_agg(',
  "jsonb_build_object('entity_ids'",
  "'values'",
  'ORDER BY "mp0_l1".ordinal ASC, "mp0_t1".entity_id ASC, "mp0_t1".locale_key ASC',
  'LIMIT $12',
]);
for (const forbidden of [
  '00000000-0000-0000-0000-000000000001',
  'rustok-product',
  'product',
  'variant',
  'variants',
  'en-US',
]) {
  if (sql.includes(forbidden)) fail(`${sqlPath} interpolates contract value ${forbidden}`);
}
if ((sql.match(/\$\d+/gu) ?? []).at(-1) !== '$12') {
  fail(`${sqlPath} must retain the canonical ordered placeholder envelope through $12`);
}

const compiledPath = 'crates/rustok-index/src/application/snapshots/m4_many_projection.compiled.snap';
const compiled = requireMarkers(compiledPath, [
  'bind:1={"type":"uuid","value":"00000000-0000-0000-0000-000000000001"}',
  'bind:7={"type":"text","value":"variants"}',
  'bind:12={"type":"integer","value":2}',
  'column:entity_id|__t0_entity_id|t0',
  'column:field|f0|id|t0',
  'many_column:__many_0|path=variants|identities=variants|fields=variants.id',
]);
if (!compiled.endsWith('\n')) fail(`${compiledPath} must end with one newline`);

requireMarkers('crates/rustok-index/src/application/mod.rs', [
  'mod query_snapshot_tests;',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-query-planner.mjs'",
  "'verify-index-postgres-query-compiler.mjs'",
  "'verify-index-query-result-decoder.mjs'",
  "'verify-index-many-link-filtering.mjs'",
  "'verify-index-query-snapshots.mjs'",
  "console.log('[verify-index-query-contract] OK')",
]);
requireNormalizedMarkers('crates/rustok-index/docs/m4-query-snapshots.md', [
  'Status: `source_complete_owner_execution_pending`',
  'compares all three files byte-for-byte',
  'does not execute SQL',
  'does not claim PostgreSQL/reference-engine equivalence',
  'verify-index-query-contract.mjs',
  'Not run by the implementation agent',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- [x] Add retained v4 plan/SQL snapshots and synchronized source guards.',
  '- [ ] Execute PostgreSQL/reference-engine equivalence capture and admit retained live evidence.',
  'M4 retained plan/SQL snapshots: `source_complete`',
]);

console.log('[verify-index-query-snapshots] OK');
