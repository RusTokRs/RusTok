#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-decimal-aggregate-wire] ${message}`);
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

const domainPath = 'crates/rustok-index/src/domain/value.rs';
requireMarkers(domainPath, [
  'Decimal(Decimal)',
  'decimal_tagged_json_uses_exact_string_wire',
  'Decimal::new(1_234_500, 4)',
  '"value": "123.4500"',
  'serde_json::from_value(encoded.clone())',
  'serde_json::to_value(decoded)',
]);

const validationPath = 'crates/rustok-index/src/application/aggregate_ordering.rs';
requireMarkers(validationPath, [
  'IndexValueType::Integer',
  '| IndexValueType::Decimal',
  '| IndexValueType::String',
  '| IndexValueType::Timestamp',
  'accepts_explicit_min_and_max_over_many_link',
]);

const compilerPath = 'crates/rustok-index/src/application/postgres_compiler.rs';
requireMarkers(compilerPath, [
  'fn aggregate_type_supported',
  'IndexValueType::Integer',
  '| IndexValueType::Decimal',
  '| IndexValueType::String',
  '| IndexValueType::Timestamp',
]);

const sqlPath = 'crates/rustok-index/src/application/postgres_query_sql.rs';
const sql = requireMarkers(sqlPath, [
  'IndexValueType::Decimal => format!("({scalar_text})::numeric")',
  'fn aggregate_order_wire_value(',
  'IndexValueType::Decimal => format!("to_jsonb(({scalar})::text)")',
  '_ => format!("to_jsonb({scalar})")',
  'let wire_value = aggregate_order_wire_value(field.value_type, &scalar);',
  "jsonb_build_object('type', '{}', 'value', {wire_value})",
]);
forbidMarkers(sqlPath, sql, [
  '::double precision',
  '::real',
  'f64',
  'to_jsonb(({scalar})::float',
]);

requireMarkers('crates/rustok-index/src/application/aggregate_ordering_tests.rs', [
  'decimal_aggregate_uses_numeric_order_and_exact_string_wire',
  'registry(IndexValueType::Decimal)',
  'jsonb_build_object(\'type\', \'decimal\', \'value\', to_jsonb(((SELECT MAX(',
  ')::text)) END AS \\"__order_0\\"',
  'aggregate_cursor_and_uuid_modes_fail_closed',
]);

const contractPath = 'crates/rustok-index/contracts/m4-decimal-aggregate-order-wire.json';
const contract = JSON.parse(read(contractPath));
if (contract.schema_version !== 1 || contract.owner !== 'rustok-index') {
  fail(`${contractPath} identity drifted`);
}
if (contract.status !== 'source_complete_execution_pending') {
  fail(`${contractPath} must remain execution pending`);
}
if (contract.domain_wire?.json_value_kind !== 'string'
  || contract.domain_wire?.example?.type !== 'decimal'
  || contract.domain_wire?.example?.value !== '123.4500') {
  fail(`${contractPath} domain wire drifted`);
}
if (contract.postgresql_wire?.typed_scalar !== 'numeric'
  || contract.postgresql_wire?.json_number_allowed !== false
  || contract.postgresql_wire?.float_conversion_allowed !== false
  || contract.postgresql_wire?.ordering_uses_text !== false
  || contract.postgresql_wire?.ordering_uses_numeric !== true) {
  fail(`${contractPath} PostgreSQL wire drifted`);
}
if (contract.query_boundary?.pagination !== 'bounded_offset'
  || contract.query_boundary?.aggregate_cursor_supported !== false) {
  fail(`${contractPath} query boundary drifted`);
}
for (const key of ['cargo_run', 'tests_run', 'postgresql_run', 'node_verifiers_run', 'workflows_run', 'ci_run']) {
  if (contract.validation?.[key] !== false) fail(`${contractPath} must not claim ${key}`);
}

const aggregateContractPath = 'crates/rustok-index/contracts/m4-many-link-aggregate-ordering.json';
const aggregateContract = JSON.parse(read(aggregateContractPath));
if (!aggregateContract.supported_terminal_types?.includes('decimal')
  || aggregateContract.rejected_terminal_types?.includes('decimal')
  || aggregateContract.remaining?.includes('exact decimal tagged-order wire contract')) {
  fail(`${aggregateContractPath} does not activate the exact Decimal wire contract`);
}
if (aggregateContract.postgresql?.decimal_order_scalar !== 'numeric'
  || aggregateContract.postgresql?.decimal_tagged_value !== 'json_string_from_numeric_text') {
  fail(`${aggregateContractPath} Decimal PostgreSQL boundary drifted`);
}

requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-decimal-aggregate-wire.mjs'",
]);
requireMarkers('crates/rustok-index/docs/m4-decimal-aggregate-order-wire.md', [
  'Status: `source_complete_execution_pending`',
  '`numeric` for `MIN/MAX`',
  '`to_jsonb((aggregate_scalar)::text)`',
  'JSON value is a string',
  'No float conversion',
  'Not run by the implementation agent',
]);

console.log('[verify-index-decimal-aggregate-wire] OK');
