#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (filename) => readFileSync(new URL(filename, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-storage-read-ordering-contract] ${message}`);
  process.exit(1);
};

const benchmarkModule = read('ops/benches/src/index_storage/mod.rs');
const benchmarkBinary = read('ops/benches/src/bin/index_storage_benchmark.rs');
const benchmarkRunner = read('ops/benches/src/index_storage/runner.rs');
const connection = read('ops/benches/src/index_storage/connection.rs');
const preflight = read('scripts/verify/check-index-storage-read-ordering.mjs');
const fixture = read('scripts/verify/check-index-storage-read-ordering.test.mjs');
const comparatorFixture = read('scripts/verify/compare-index-storage-evidence.test.mjs');
const validatorWrapper = read('scripts/verify/validate-index-storage-evidence.mjs');
const comparatorWrapper = read('scripts/verify/compare-index-storage-evidence.mjs');
const standaloneFixture = read('scripts/verify/index-storage-standalone-tools.test.mjs');
const standaloneGuard = read('scripts/verify/verify-index-storage-standalone-tools.mjs');
const router = read('scripts/verify/index-storage-tooling.mjs');
const routerFixture = read('scripts/verify/index-storage-tooling.test.mjs');
const smokeWorkflow = read('.github/workflows/index-storage-smoke.yml');
const scaleWorkflow = read('.github/workflows/index-storage-scale-evidence.yml');
const scaleRunWorkflow = read('.github/workflows/index-storage-scale-run.yml');

const requireMarkers = (content, label, markers) => {
  for (const marker of markers) {
    if (!content.includes(marker)) fail(`${label} is missing contract marker: ${marker}`);
  }
};

const sessionMetadataMarkers = [
  "standard_conforming_strings: 'on'",
  "timezone: 'UTC'",
  "date_style: 'ISO, YMD'",
  "extra_float_digits: '3'",
];

requireMarkers(connection, 'benchmark session contract', [
  'pub(crate) const BENCHMARK_SESSION_METADATA',
  '("standard_conforming_strings", "on")',
  '("timezone", "UTC")',
  '("date_style", "ISO, YMD")',
  '("extra_float_digits", "3")',
  'SET standard_conforming_strings = on;',
  "SET TIME ZONE 'UTC';",
  "SET DateStyle = 'ISO, YMD';",
  'SET extra_float_digits = 3;',
  'failed to pin deterministic PostgreSQL benchmark session',
  'BENCHMARK_SESSION_METADATA.contains(&(field, value))',
]);

requireMarkers(benchmarkRunner, 'observed database metadata', [
  'const DATABASE_METADATA_SQL',
  'pub struct DatabaseMetadata',
  'pub standard_conforming_strings: String',
  'pub timezone: String',
  'pub date_style: String',
  'pub extra_float_digits: String',
  "current_setting('standard_conforming_strings') AS standard_conforming_strings",
  "current_setting('TimeZone') AS timezone",
  "current_setting('DateStyle') AS date_style",
  "current_setting('extra_float_digits') AS extra_float_digits",
  'standard_conforming_strings: row.try_get("", "standard_conforming_strings")?',
  'timezone: row.try_get("", "timezone")?',
  'date_style: row.try_get("", "date_style")?',
  'extra_float_digits: row.try_get("", "extra_float_digits")?',
  'fn database_metadata_query_observes_deterministic_session()',
]);
requireMarkers(benchmarkModule, 'benchmark report writer', [
  'pub use runner::{BenchmarkReport, run, write_report};',
  'write_report(&config.output_path, &report)?',
]);
requireMarkers(benchmarkBinary, 'read evidence executable', [
  'BenchmarkConfig, run, write_report',
  'write_report(&config.output_path, &report)?',
]);
for (const [label, content] of [
  ['benchmark module', benchmarkModule],
  ['read evidence executable', benchmarkBinary],
]) {
  for (const forbidden of [
    'write_report_with_session_metadata',
    'serde_json::to_value(report)',
    'database.insert(field.to_owned()',
  ]) {
    if (content.includes(forbidden)) {
      fail(`${label} restored post-hoc deterministic session metadata injection: ${forbidden}`);
    }
  }
}

requireMarkers(preflight, 'read ordering preflight', [
  "const canonicalPrototypes = ['jsonb', 'typed_eav', 'hot_projection']",
  "'status_equality'",
  "'price_range_sort'",
  "'multi_value_tag'",
  "'two_hop_channel_filter'",
  "'keyset_page'",
  "'exact_count'",
  'const canonicalSessionMetadata = new Map',
  "['standard_conforming_strings', 'on']",
  "['timezone', 'UTC']",
  "['date_style', 'ISO, YMD']",
  "['extra_float_digits', '3']",
  'const requireSessionMetadata = (read, directory) =>',
  'requireSessionMetadata(read, directory);',
  'deterministic session metadata and executable terminal ordering verified',
  'const maskSqlText = (text) =>',
  'const identifierContinuation = /[A-Za-z0-9_$]/u',
  'const isEscapeStringQuote = (sql, quoteIndex) =>',
  'const executableSqlText = (sql, label) =>',
  "sql.startsWith('--', index)",
  "sql.startsWith('/*', index)",
  'unterminated block comment',
  "const escapeString = quote === \"'\" && isEscapeStringQuote(sql, index)",
  "escapeString && sql[index] === '\\\\'",
  'unterminated escape string literal',
  "? (escapeString ? 'escape string literal' : 'string literal')",
  'contains an unterminated ${kind}',
  'unterminated dollar-quoted string',
  'executableSql.trimEnd().endsWith(marker)',
  'must end with canonical ordering marker',
  'in executable SQL',
  'validatePacketReadOrdering',
  'source workload order',
  'prototype order',
]);
for (const forbidden of ['sql.includes(marker)', 'sql.trimEnd().endsWith(marker)']) {
  if (preflight.includes(forbidden)) {
    fail(`read ordering preflight restored unsafe raw-SQL validation: ${forbidden}`);
  }
}

requireMarkers(fixture, 'read ordering fixture', [
  ...sessionMetadataMarkers,
  "test('accepts canonical terminal ordering with trailing whitespace'",
  "test('rejects missing deterministic session metadata'",
  "test('rejects deterministic session metadata drift'",
  "test('accepts comment tokens inside strings and comments after executable ordering'",
  "test('rejects a source ordering marker that exists only in a nested query'",
  "test('rejects a candidate ordering marker that exists only in a block comment'",
  "test('rejects a terminal ordering marker hidden in a line comment'",
  "test('rejects an ordering marker hidden in a dollar-quoted string'",
  "test('rejects an ordering marker hidden after an escaped quote in an E string'",
  "test('rejects unterminated SQL comments before ordering validation'",
  "test('rejects workload order drift before checking SQL text'",
]);
requireMarkers(comparatorFixture, 'comparison evidence fixture', [
  ...sessionMetadataMarkers,
  'function writePacket(root, scale, overrides = {})',
  "test('same-commit complete 100k and 1m evidence is decision-ready'",
]);

requireMarkers(validatorWrapper, 'standalone validator wrapper', [
  "import { validatePacketReadOrdering } from './check-index-storage-read-ordering.mjs'",
  'validatePacketReadOrdering(evidenceRoot);',
  "await import('./validate-index-storage-evidence-core.mjs')",
]);
const validatorOrdering = validatorWrapper.indexOf('validatePacketReadOrdering(evidenceRoot);');
const validatorCore = validatorWrapper.indexOf("await import('./validate-index-storage-evidence-core.mjs')");
if (validatorOrdering < 0 || validatorCore < 0 || validatorOrdering > validatorCore) {
  fail('standalone validator must preflight ordering before importing its core');
}

requireMarkers(comparatorWrapper, 'standalone comparator wrapper', [
  "import { validatePacketReadOrdering } from './check-index-storage-read-ordering.mjs'",
  'for (const input of inputs) validatePacketReadOrdering(input);',
  "await import('./compare-index-storage-evidence-core.mjs')",
]);
const standaloneCompareOrdering = comparatorWrapper.indexOf(
  'for (const input of inputs) validatePacketReadOrdering(input);',
);
const standaloneComparatorCore = comparatorWrapper.indexOf(
  "await import('./compare-index-storage-evidence-core.mjs')",
);
if (standaloneCompareOrdering < 0 || standaloneComparatorCore < 0
    || standaloneCompareOrdering > standaloneComparatorCore) {
  fail('standalone comparator must preflight every input before importing its core');
}

requireMarkers(standaloneFixture, 'standalone evidence fixture', [
  ...sessionMetadataMarkers,
  "test('direct validator rejects non-executable terminal ordering before its core'",
  "test('direct comparator rejects nested-only terminal ordering before its core'",
  "test('direct validator reaches the byte-preserved core after a valid preflight'",
  "test('direct comparator forwards help to the byte-preserved core'",
  'assert.doesNotMatch(result.stderr, missingMutation)',
]);
requireMarkers(standaloneGuard, 'standalone evidence guard', [
  "const gitBlobSha = (bytes) => createHash('sha1')",
  "'dabc18d59360c300352ab3afb2510f0a0ff22796'",
  "'97ef0e8a216735e457c4c827d975462b84b009b3'",
  'validator wrapper must run terminal-ordering preflight before importing its core',
  'comparator wrapper must preflight every input before importing its core',
]);

requireMarkers(router, 'storage tooling router', [
  "'verify-index-storage-read-ordering-contract.mjs'",
  "'verify-index-storage-standalone-tools.mjs'",
  "scriptPath('check-index-storage-read-ordering.test.mjs')",
  "scriptPath('index-storage-standalone-tools.test.mjs')",
  "runScript('check-index-storage-read-ordering.mjs', ['--input', packetRoot])",
  "runScript('check-index-storage-read-ordering.mjs', orderingArgs)",
  "runScript('validate-index-storage-evidence.mjs', [], environment)",
  "runScript('compare-index-storage-evidence.mjs', args)",
]);
const packetOrdering = router.indexOf("runScript('check-index-storage-read-ordering.mjs', ['--input', packetRoot])");
const packetValidator = router.indexOf("runScript('validate-index-storage-evidence.mjs', [], environment)");
if (packetOrdering < 0 || packetValidator < 0 || packetOrdering > packetValidator) {
  fail('packet terminal ordering preflight must run before the canonical validator');
}
const compareOrdering = router.indexOf("runScript('check-index-storage-read-ordering.mjs', orderingArgs)");
const comparator = router.indexOf("runScript('compare-index-storage-evidence.mjs', args)");
if (compareOrdering < 0 || comparator < 0 || compareOrdering > comparator) {
  fail('comparison terminal ordering preflight must run before the canonical comparator');
}

requireMarkers(routerFixture, 'storage tooling router fixture', [
  ...sessionMetadataMarkers,
  "test('packet runs terminal ordering preflight before the canonical validator'",
  "test('compare runs terminal ordering preflight before the canonical comparator'",
  'assert.doesNotMatch(result.stderr, /missing evidence file: .*mutation-report\\.json/u)',
]);

for (const [label, workflow] of [
  ['smoke workflow', smokeWorkflow],
  ['scale workflow', scaleWorkflow],
]) {
  requireMarkers(workflow, label, [
    'scripts/verify/check-index-storage-read-ordering.mjs',
    'scripts/verify/check-index-storage-read-ordering.test.mjs',
    'scripts/verify/verify-index-storage-read-ordering-contract.mjs',
    'scripts/verify/validate-index-storage-evidence-core.mjs',
    'scripts/verify/compare-index-storage-evidence-core.mjs',
    'scripts/verify/index-storage-standalone-tools.test.mjs',
    'scripts/verify/verify-index-storage-standalone-tools.mjs',
    'node --check scripts/verify/check-index-storage-read-ordering.mjs',
    'node --check scripts/verify/check-index-storage-read-ordering.test.mjs',
    'node --check scripts/verify/index-storage-standalone-tools.test.mjs',
    'node --check scripts/verify/verify-index-storage-standalone-tools.mjs',
  ]);
}
requireMarkers(scaleRunWorkflow, 'scale run workflow', [
  'node scripts/verify/index-storage-tooling.mjs packet',
]);

console.log('[verify-index-storage-read-ordering-contract] observed PostgreSQL session metadata, canonical evidence writer, session-complete fixtures, executable SQL lexer, PostgreSQL escape strings, standalone entrypoints, public command order, and workflows are consistent');
