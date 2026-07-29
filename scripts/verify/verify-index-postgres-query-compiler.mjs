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
  'pub struct CompiledPostgresQuery',
  'pub enum PostgresQueryCompileError',
  'pub fn compile_postgres(&self)',
  'self.validate_compiler_subset()?;',
  'PostgresBindValue::Uuid(self.scope.tenant_id)',
  'PostgresBindValue::Text(join.link.as_str().to_owned())',
  'PostgresBindValue::Text(field.path.field().as_str().to_owned())',
  'format!("${}", self.values.len())',
  'LEFT JOIN index_links AS',
  '.source_version =',
  '.target_schema_version =',
  'LEFT JOIN index_entities AS',
  '.is_deleted = FALSE',
  'jsonb_extract_path(',
  'ORDER BY {root_alias}.entity_id ASC LIMIT {limit}',
  'FilterPending',
  'OrderingPending',
  'ExactCountPending',
  'CursorContinuationPending',
  'OffsetPaginationPending',
  'ManyLinkProjectionPending',
  'AliasMappingMismatch',
  'self.path_aliases.get(&Vec::new())',
  'validate_alias(&self.root_alias)?;',
  'quote_identifier',
]);

const validatePosition = compiler.indexOf('self.validate_compiler_subset()?;');
const tenantBindPosition = compiler.indexOf('PostgresBindValue::Uuid(self.scope.tenant_id)');
const joinBindPosition = compiler.indexOf('PostgresBindValue::Text(join.link.as_str().to_owned())');
const fieldBindPosition = compiler.indexOf(
  'PostgresBindValue::Text(\n                field.path.field().as_str().to_owned()',
);
const limitBindPosition = compiler.indexOf(
  'bindings.push(PostgresBindValue::Integer(i64::from(*first)))',
);
const sqlPosition = compiler.indexOf('let mut sql = format!(');
if (validatePosition < 0 || sqlPosition < 0 || validatePosition >= sqlPosition) {
  fail('compiler subset validation must precede SQL construction');
}
if (
  tenantBindPosition < 0 ||
  joinBindPosition < 0 ||
  fieldBindPosition < 0 ||
  limitBindPosition < 0 ||
  !(
    tenantBindPosition < joinBindPosition &&
    joinBindPosition < fieldBindPosition &&
    fieldBindPosition < limitBindPosition &&
    limitBindPosition < sqlPosition
  )
) {
  fail('scope, join, projection, and limit binds must remain deterministically ordered');
}

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
  if (compiler.includes(forbidden)) fail(`${compilerPath} contains forbidden marker ${forbidden}`);
}

requireMarkers('crates/rustok-index/src/application/postgres_compiler_tests.rs', [
  'compiles_root_projection_with_bound_scope_and_limit',
  'compiles_one_link_projection_without_interpolating_contract_values',
  'rejects_semantics_reserved_for_follow_up_compiler_slices',
  'rejects_many_link_projection_before_sql_is_emitted',
  'rejects_tampered_path_alias_mapping',
  'assert!(!compiled.sql.contains(&tenant_id.to_string()))',
  'assert!(!compiled.sql.contains("sales_channel"))',
]);
requireMarkers('crates/rustok-index/src/application/mod.rs', [
  'mod postgres_compiler;',
  'mod postgres_compiler_tests;',
  'CompiledPostgresQuery',
  'PostgresQueryCompileError',
]);
requireMarkers('crates/rustok-index/docs/m4-postgres-query-compiler.md', [
  'controlled PostgreSQL statement plus an ordered typed bind list',
  'projection through explicit one-cardinality links',
  'Fail-closed pending semantics',
  'does not execute SQL',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  'M4 controlled PostgreSQL query compilation: `complete`',
  '- [x] Compile plans through SeaQuery or controlled SQL.',
  'Typed filter/order/count/keyset compilation remains the next bounded M4 slice.',
]);

console.log('[verify-index-postgres-query-compiler] OK');
