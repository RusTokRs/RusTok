#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-postgres-reference-equivalence] ${message}`);
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
const requirePattern = (relative, pattern, description) => {
  const source = read(relative);
  if (!pattern.test(source)) fail(`${relative} is missing ${description}`);
  return source;
};

const testPath = 'crates/rustok-index/src/infrastructure/postgres/postgres_reference_equivalence_tests.rs';
const test = requireMarkers(testPath, [
  'RUSTOK_INDEX_TEST_DATABASE_URL',
  'CREATE SCHEMA',
  'SET search_path',
  'IndexModule.migrations()',
  'PostgresSchemaRegistrationStore',
  'PostgresMutationStore',
  'PostgresIndexQueryPort',
  'ReferenceFixture::new',
  'postgres_query_port_matches_reference_fixture',
  'assert_eq!(actual, expected);',
  'FilterExpr::Contains(',
  'FilterExpr::Ne(',
  'FilterExpr::IsNull(',
  'Pagination::Offset',
  'DROP SCHEMA IF EXISTS',
]);
for (const forbidden of [
  'INSERT INTO index_schemas',
  'INSERT INTO index_entities',
  'INSERT INTO index_links',
  'testcontainers',
  'GenericImage',
  'SELECT *',
]) {
  if (test.includes(forbidden)) fail(`${testPath} contains forbidden marker ${forbidden}`);
}

const referencePath = 'crates/rustok-index/src/infrastructure/postgres/postgres_reference_equivalence_tests/reference_fixture.rs';
requireMarkers(referencePath, [
  'CursorCodec::decode_scoped_for_query',
  'CursorCodec::encode_for_query',
  'let exact_count = query.include_exact_count.then_some(records.len() as u64);',
  '.take(page_size + 1)',
  'FilterExpr::And(children)',
  'FilterExpr::Or(children)',
  'FilterExpr::Not(child)',
  'FilterExpr::Eq(path, expected)',
  'FilterExpr::Ne(path, expected)',
  'FilterExpr::In(path, expected)',
  'FilterExpr::Gt(path, expected)',
  'FilterExpr::Gte(path, expected)',
  'FilterExpr::Lt(path, expected)',
  'FilterExpr::Lte(path, expected)',
  'FilterExpr::Contains(path, expected)',
  'FilterExpr::IsNull(path, expected_null)',
]);
requirePattern(referencePath, /plan\s*\.outer_projection\(\)/u, 'plan.outer_projection()');
requirePattern(referencePath, /plan\s*\.many_projections\b/u, 'plan.many_projections');

requireMarkers('crates/rustok-index/src/infrastructure/postgres/mod.rs', [
  'mod postgres_reference_equivalence_tests;',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-postgres-reference-equivalence.mjs'",
]);
requireNormalizedMarkers('crates/rustok-index/docs/m4-postgres-reference-equivalence.md', [
  'Status: `fixture_capture_and_admission_source_complete_owner_execution_pending`',
  'compares the complete `IndexQueryPage`',
  'does not introduce Testcontainers or a second database stack',
  'Not run by the implementation agent',
]);
requireNormalizedMarkers('crates/rustok-index/docs/m4-query-planner.md', [
  'M4 PostgreSQL/reference fixture source: `source_complete_owner_execution_pending`',
  'M4 live PostgreSQL/reference execution evidence: `open_owner_action`',
  'The canonical checklist remains open until the owner runs the PostgreSQL/reference fixture through capture',
]);

console.log('[verify-index-postgres-reference-equivalence] OK');
