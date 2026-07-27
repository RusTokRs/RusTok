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
const databaseMetadata = read('ops/benches/src/index_storage/database_metadata.rs');
const mutationRunner = read('ops/benches/src/index_storage/mutation_runner.rs');
const maintenanceRunner = read('ops/benches/src/index_storage/maintenance_runner.rs');
const reportProvenance = read('ops/benches/src/index_storage/report_provenance.rs');
const connection = read('ops/benches/src/index_storage/connection.rs');
const preflight = read('scripts/verify/check-index-storage-read-ordering.mjs');
const fixture = read('scripts/verify/check-index-storage-read-ordering.test.mjs');
const comparatorFixture = read('scripts/verify/compare-index-storage-evidence.test.mjs');
const databaseSettingsContract = read('scripts/verify/index-storage-database-settings-contract.mjs');
const prepareDecision = read('scripts/verify/prepare-index-storage-decision.mjs');
const finalizeAdr = read('scripts/verify/finalize-index-storage-adr.mjs');
const decisionFixture = read('scripts/verify/index-storage-decision-tooling.test.mjs');
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
const comparableDatabaseFieldMarkers = [
  "'server_version_num'",
  "'shared_buffers'",
  "'effective_cache_size'",
  "'work_mem'",
  "'random_page_cost'",
  "'jit'",
  "'standard_conforming_strings'",
  "'timezone'",
  "'date_style'",
  "'extra_float_digits'",
];

requireMarkers(connection, 'benchmark session contract', [
  'const BENCHMARK_SESSION_SQL',
  'SET standard_conforming_strings = on;',
  "SET TIME ZONE 'UTC';",
  "SET DateStyle = 'ISO, YMD';",
  'SET extra_float_digits = 3;',
  'failed to pin deterministic PostgreSQL benchmark session',
  'fn benchmark_session_contract_is_explicit_and_deterministic()',
]);

requireMarkers(databaseMetadata, 'shared observed database metadata', [
  'const DATABASE_METADATA_SQL',
  '#[derive(Debug, Clone, PartialEq, Eq, Serialize)]',
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
  'pub(crate) async fn read_database_metadata',
  'pub(crate) async fn ensure_database_metadata_stable',
  'fn database_metadata_query_observes_comparable_session_settings()',
]);
for (const [label, content, benchmark] of [
  ['read runner', benchmarkRunner, 'read benchmark'],
  ['mutation runner', mutationRunner, 'mutation benchmark'],
  ['maintenance runner', maintenanceRunner, 'maintenance benchmark'],
]) {
  requireMarkers(content, `${label} shared database metadata`, [
    'DatabaseMetadata',
    'read_database_metadata',
    'ensure_database_metadata_stable',
    'pub database: DatabaseMetadata',
    `ensure_database_metadata_stable(&db, &database, "${benchmark}").await?;`,
  ]);
  for (const forbidden of [
    'const DATABASE_METADATA_SQL',
    'pub struct DatabaseMetadata',
    'async fn read_database_metadata',
  ]) {
    if (content.includes(forbidden)) {
      fail(`${label} duplicated shared PostgreSQL metadata ownership: ${forbidden}`);
    }
  }
}
requireMarkers(benchmarkModule, 'benchmark report writer', [
  'pub use database_metadata::DatabaseMetadata;',
  'ensure_database_metadata_stable, read_database_metadata',
  'pub use report_provenance::write_provenance_bound_report;',
  'write_provenance_bound_report(&config.output_path, &report, &config.run_provenance)?',
]);
requireMarkers(benchmarkBinary, 'read evidence executable', [
  'BenchmarkConfig, run, write_provenance_bound_report',
  'write_provenance_bound_report(&config.output_path, &report, &config.run_provenance)?',
]);
requireMarkers(reportProvenance, 'provenance-bound report publication', [
  '#[serde(flatten)]',
  "provenance: &'a BenchmarkRunProvenance",
  'pub fn write_provenance_bound_report<T: Serialize>',
  'let staged = path.with_file_name',
  'fs::write(&staged, serde_json::to_vec_pretty(&envelope)?)',
  'fs::rename(&staged, path)',
  'if staged.exists()',
]);
for (const [label, content] of [
  ['benchmark module', benchmarkModule],
  ['read evidence executable', benchmarkBinary],
]) {
  for (const forbidden of [
    'write_report(&config.output_path, &report)?',
    'write_report_with_session_metadata',
    'serde_json::to_value(report)',
    'database.insert(field.to_owned()',
  ]) {
    if (content.includes(forbidden)) {
      fail(`${label} restored unbound or post-hoc report publication: ${forbidden}`);
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
  'benchmark run provenance, deterministic session metadata, and executable terminal ordering verified',
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
  "test('rejects mutation database metadata drift from the read session'",
  "test('rejects maintenance report without exact database metadata fields'",
  "test('rejects a mutation report without observed database metadata'",
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
  ...comparableDatabaseFieldMarkers,
  'function writePacket(root, scale, overrides = {})',
  "test('same-commit complete 100k and 1m evidence is decision-ready'",
  'assert.deepEqual(report.methodology.comparable_database_fields, comparableDatabaseFields);',
  'database_settings_source',
  "test('rejects observed session metadata drift before comparison output is accepted'",
  "test('rejects mutation session metadata drift within one packet'",
]);

requireMarkers(databaseSettingsContract, 'shared database settings contract', [
  'export const comparableDatabaseFields = Object.freeze([',
  ...comparableDatabaseFieldMarkers,
  'export const databaseSettingsSource =',
  'read-report.json database metadata observed from the active PostgreSQL benchmark session',
  'export const requireComparisonDatabaseSettingsMethodology = (comparison, fail) =>',
  'comparable_database_fields must exactly match the canonical PostgreSQL database-settings contract',
  'comparison methodology database_settings_source must identify read metadata observed from the active PostgreSQL benchmark session after exact equality with mutation and maintenance active-session metadata',
]);
requireMarkers(prepareDecision, 'decision preparation database settings gate', [
  "from './index-storage-database-settings-contract.mjs'",
  'requireComparisonDatabaseSettingsMethodology(comparison, fail);',
]);
requireMarkers(finalizeAdr, 'ADR finalization database settings gate', [
  "from './index-storage-database-settings-contract.mjs'",
  'requireComparisonDatabaseSettingsMethodology(comparison.value, fail);',
]);
requireMarkers(decisionFixture, 'decision tooling database settings fixture', [
  ...comparableDatabaseFieldMarkers,
  'databaseSettingsSource',
  'comparable_database_fields: [...comparableDatabaseFields]',
  "test('rejects a comparison without the canonical database-settings methodology'",
  "test('finalizer rejects database-settings provenance drift with a matching comparison digest'",
]);
const prepareDatabaseGate = prepareDecision.indexOf(
  'requireComparisonDatabaseSettingsMethodology(comparison, fail);',
);
const prepareDecisionWrite = prepareDecision.indexOf('const decision = {');
if (prepareDatabaseGate < 0 || prepareDecisionWrite < 0 || prepareDatabaseGate > prepareDecisionWrite) {
  fail('decision preparation must validate database-settings methodology before creating a decision');
}
const finalizeDatabaseGate = finalizeAdr.indexOf(
  'requireComparisonDatabaseSettingsMethodology(comparison.value, fail);',
);
const finalizeRenderer = finalizeAdr.indexOf("path.join(scriptDirectory, 'render-index-storage-adr.mjs')");
if (finalizeDatabaseGate < 0 || finalizeRenderer < 0 || finalizeDatabaseGate > finalizeRenderer) {
  fail('ADR finalization must validate database-settings methodology before invoking the renderer');
}

requireMarkers(validatorWrapper, 'standalone validator wrapper', [
  "import { validatePacketReadOrdering } from './check-index-storage-read-ordering.mjs'",
  "import { runValidatorCoreWithAtomicProvenance } from './run-index-storage-validator-core.mjs'",
  "import { validateRunnerResourceSnapshots } from './validate-index-storage-runner-resources.mjs'",
  "const corePath = fileURLToPath(new URL('./validate-index-storage-evidence-core.mjs', import.meta.url))",
  'runValidatorCoreWithAtomicProvenance({ evidenceRoot, corePath })',
]);
const validatorInvalidation = validatorWrapper.indexOf('invalidatePacketProvenance(evidenceRoot);');
const validatorOrdering = validatorWrapper.indexOf('validatePacketReadOrdering(evidenceRoot);');
const validatorResources = validatorWrapper.indexOf('validateRunnerResourceSnapshots(evidenceRoot);');
const validatorCore = validatorWrapper.indexOf('runValidatorCoreWithAtomicProvenance({ evidenceRoot, corePath })');
if ([validatorInvalidation, validatorOrdering, validatorResources, validatorCore].some((index) => index < 0)
    || !(validatorInvalidation < validatorOrdering
      && validatorOrdering < validatorResources
      && validatorResources < validatorCore)) {
  fail('standalone validator must revoke stale provenance, preflight ordering/resources, then run its isolated core');
}

requireMarkers(comparatorWrapper, 'standalone comparator wrapper', [
  "import { validatePacketReadOrdering } from './check-index-storage-read-ordering.mjs'",
  "from './index-storage-database-settings-contract.mjs'",
  'comparableDatabaseFields,',
  'databaseSettingsSource,',
  'const finalizeDatabaseSettingsContract = ({ inputs, output }) =>',
  "const corePath = fileURLToPath(new URL('./compare-index-storage-evidence-core.mjs', import.meta.url))",
  'for (const input of parsed.inputs) validatePacketReadOrdering(input);',
  'cross-scale database setting ${field} mismatch',
  'methodology.comparable_database_fields = comparableDatabaseFields;',
  'methodology.database_settings_source = databaseSettingsSource;',
  'Compared PostgreSQL fields:',
  'runComparatorCoreWithAtomicComparison(parsed)',
]);
const standaloneComparisonRevocation = comparatorWrapper.indexOf(
  "rmSync(path.join(parsed.output, 'comparison.json'), { force: true });",
);
const standaloneCompareOrdering = comparatorWrapper.indexOf(
  'for (const input of parsed.inputs) validatePacketReadOrdering(input);',
);
const standaloneComparatorCore = comparatorWrapper.indexOf(
  'runComparatorCoreWithAtomicComparison(parsed)',
);
if ([standaloneComparisonRevocation, standaloneCompareOrdering, standaloneComparatorCore]
  .some((index) => index < 0)
    || !(standaloneComparisonRevocation < standaloneCompareOrdering
      && standaloneCompareOrdering < standaloneComparatorCore)) {
  fail('standalone comparator must revoke stale JSON, preflight every input, then run its atomic lifecycle');
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
  'runValidatorCoreWithAtomicProvenance',
  'runComparatorCoreWithAtomicComparison',
  'has lifecycle markers out of order',
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

requireMarkers(smokeWorkflow, 'smoke workflow', [
  'scripts/verify/check-index-storage-read-ordering.mjs',
  'scripts/verify/check-index-storage-read-ordering.test.mjs',
  'scripts/verify/verify-index-storage-read-ordering-contract.mjs',
  'scripts/verify/validate-index-storage-evidence-core.mjs',
  'scripts/verify/compare-index-storage-evidence-core.mjs',
  'scripts/verify/index-storage-standalone-tools.test.mjs',
  'scripts/verify/verify-index-storage-standalone-tools.mjs',
  'node --check scripts/verify/check-index-storage-read-ordering.mjs',
  'node --check scripts/verify/verify-index-storage-standalone-tools.mjs',
]);
requireMarkers(scaleWorkflow, 'scale workflow', [
  'scripts/verify/*index-storage*.mjs',
  'scripts/verify/storage-decision*.mjs',
  'scripts/verify/*methodology-envelope*.mjs',
  'find scripts/verify -maxdepth 1 -type f',
  'node scripts/verify/index-storage-tooling.mjs contract',
  'node --test scripts/verify/index-storage-validator-arguments.test.mjs',
  'node scripts/verify/index-storage-tooling.mjs fixtures',
  "if: ${{ github.event_name == 'workflow_dispatch' }}",
]);
requireMarkers(scaleRunWorkflow, 'scale run workflow', [
  'node scripts/verify/index-storage-tooling.mjs packet',
]);

console.log('[verify-index-storage-read-ordering-contract] shared Rust PostgreSQL metadata ownership, start/end session stability, cross-report equality, cross-scale database settings, ADR-bound comparison methodology, session-complete fixtures, executable SQL lexer, PostgreSQL escape strings, standalone entrypoints, public command order, and workflows are consistent');
