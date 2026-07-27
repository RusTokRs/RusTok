#!/usr/bin/env node

import { readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-storage-source-oracle] ${message}`);
  process.exit(1);
};

const benchmarkModule = read('ops/benches/src/index_storage/mod.rs');
const connection = read('ops/benches/src/index_storage/connection.rs');
const explainParser = read('ops/benches/src/index_storage/explain.rs');
const sourceSql = read('ops/benches/src/index_storage/sql/source.rs');
const sqlModule = read('ops/benches/src/index_storage/sql/mod.rs');
const runner = read('ops/benches/src/index_storage/runner.rs');
const mutationRunner = read('ops/benches/src/index_storage/mutation_runner.rs');
const maintenanceRunner = read('ops/benches/src/index_storage/maintenance_runner.rs');
const validator = read('scripts/verify/validate-index-storage-evidence-core.mjs');
const comparator = read('scripts/verify/compare-index-storage-evidence-core.mjs');
const validatorWrapper = read('scripts/verify/validate-index-storage-evidence.mjs');
const comparatorWrapper = read('scripts/verify/compare-index-storage-evidence.mjs');
const comparatorFixture = read('scripts/verify/compare-index-storage-evidence.test.mjs');
const orderingPreflight = read('scripts/verify/check-index-storage-read-ordering.mjs');
const orderingFixture = read('scripts/verify/check-index-storage-read-ordering.test.mjs');
const orderingGuard = read('scripts/verify/verify-index-storage-read-ordering-contract.mjs');
const standaloneFixture = read('scripts/verify/index-storage-standalone-tools.test.mjs');
const standaloneGuard = read('scripts/verify/verify-index-storage-standalone-tools.mjs');
const toolingRouter = read('scripts/verify/index-storage-tooling.mjs');
const adrIntegrityGuard = read('scripts/verify/verify-index-storage-adr-integrity.mjs');
const smokeWorkflow = read('.github/workflows/index-storage-smoke.yml');
const scaleWorkflow = read('.github/workflows/index-storage-scale-evidence.yml');

const readWorkloads = [
  'status_equality',
  'price_range_sort',
  'multi_value_tag',
  'two_hop_channel_filter',
  'keyset_page',
  'exact_count',
];

for (const marker of [
  'mod explain;',
  'source_workloads',
  'read_workload_contract',
  'RESULT_DIGEST_CONTRACT',
]) {
  if (!benchmarkModule.includes(marker)) fail(`benchmark module missing ${marker}`);
}
for (const marker of [
  'min_connections(1)',
  'max_connections(1)',
  'SET standard_conforming_strings = on;',
  'failed to pin PostgreSQL standard-conforming string semantics',
]) {
  if (!connection.includes(marker)) {
    fail(`benchmark connection missing deterministic string contract ${marker}`);
  }
}
for (const [label, content] of [
  ['read runner', runner],
  ['mutation runner', mutationRunner],
  ['maintenance runner', maintenanceRunner],
]) {
  if (!content.includes('connect_benchmark_database(&config.database_url).await?')) {
    fail(`${label} must use the shared single-session benchmark connection`);
  }
  if (content.includes('standard_conforming_strings = off')) {
    fail(`${label} overrides deterministic PostgreSQL string semantics`);
  }
}
for (const marker of [
  'parse_read_explain_metrics',
  'parse_mutation_explain_metrics',
  'root_and_plan_node',
  'required_non_negative_f64',
  'required_direct_metric_pair',
  'required_maximum_metric_pair',
  'required_maximum_metric_triple',
  'first.unwrap_or(0)',
  'second.unwrap_or(0)',
  'third.unwrap_or(0)',
  'EXPLAIN result must contain exactly one root entry',
  'missing the {family} metric family',
  'omitted_members_of_present_metric_family_become_zero',
  'required_metric_family_fails_closed_when_absent',
]) {
  if (!explainParser.includes(marker)) fail(`Rust EXPLAIN parser missing ${marker}`);
}

for (const marker of [
  'pub fn workloads(context: &WorkloadContext) -> Vec<Workload>',
  'idx_bench_source.product',
  'idx_bench_source.variant',
  'idx_bench_source.variant_channel',
  'idx_bench_source.channel',
]) {
  if (!sourceSql.includes(marker)) fail(`source SQL oracle missing ${marker}`);
}
for (const workload of readWorkloads) {
  if (!sourceSql.includes(`name: "${workload}"`)) {
    fail(`source SQL oracle missing workload ${workload}`);
  }
}

for (const marker of [
  'pub const RESULT_DIGEST_CONTRACT: &str = "ordered_length_prefixed_json_v1"',
  'pub fn source_workloads(config: &DatasetConfig) -> Vec<Workload>',
  'source::workloads(&WorkloadContext::new(config))',
  'pub(crate) fn read_workload_contract',
  'digest_order_by',
  'sql_order_marker',
  'assert_read_workload_contract',
  'ORDER BY entity_id LIMIT 100',
  'ORDER BY price_minor, entity_id LIMIT 100',
]) {
  if (!sqlModule.includes(marker)) fail(`SQL module missing read digest contract ${marker}`);
}

for (const marker of [
  'pub result_digest_contract: &\'static str',
  'result_digest_contract: RESULT_DIGEST_CONTRACT',
  'pub source_workloads: Vec<SourceWorkloadReport>',
  'run_source_workloads(&db, &config.dataset)',
  'validate_semantic_parity(&source_workload_reports, &prototypes)',
  'parse_read_explain_metrics(&plan)',
  'read_workload_contract(workload_name).digest_order_by',
  'ORDER BY {order_by}',
  'result_json.len()',
  'SELECT md5($1::text) AS result_digest',
  'pub planning_time_ms: f64',
  'pub execution_time_ms: f64',
  'pub shared_hit_blocks: u64',
  'pub shared_read_blocks: u64',
  'differs from source oracle',
]) {
  if (!runner.includes(marker)) fail(`read runner missing evidence contract ${marker}`);
}
for (const legacy of [
  "string_agg(row_to_json(result)::text, '|'",
  'ORDER BY row_to_json(result)::text',
]) {
  if (runner.includes(legacy)) fail(`read runner restored unordered/set-like digest: ${legacy}`);
}
for (const marker of [
  'parse_mutation_explain_metrics(&plan)',
  'pub planning_time_ms: f64',
  'pub execution_time_ms: f64',
  'pub shared_hit_blocks: u64',
  'pub shared_read_blocks: u64',
  'pub maximum_node_wal_records: u64',
  'pub maximum_node_wal_fpi: u64',
  'pub maximum_node_wal_bytes: u64',
]) {
  if (!mutationRunner.includes(marker)) fail(`mutation runner missing evidence contract ${marker}`);
}
for (const legacy of [
  'pub planning_time_ms: Option<f64>',
  'pub execution_time_ms: Option<f64>',
  'pub shared_hit_blocks: Option<u64>',
  'pub shared_read_blocks: Option<u64>',
  'pub maximum_node_wal_records: Option<u64>',
  'pub maximum_node_wal_fpi: Option<u64>',
  'pub maximum_node_wal_bytes: Option<u64>',
]) {
  if (runner.includes(legacy) || mutationRunner.includes(legacy)) {
    fail(`Rust runner restored nullable required EXPLAIN metric: ${legacy}`);
  }
}

for (const marker of [
  "const resultDigestContract = 'ordered_length_prefixed_json_v1'",
  'const readOrderMarkers = new Map',
  'requireReadOrdering',
  'read.result_digest_contract',
  'result_digest_contract: resultDigestContract',
  'read.source_workloads',
  "'source workload order'",
  "sourceWorkload.sql.includes('idx_bench_source.')",
  "workload.sql.includes('idx_bench_source.')",
  'RFC 3339 UTC timestamp',
  'server_version_num must contain only digits',
  'differs from source oracle',
  'source_workload_names: canonicalReadWorkloads',
]) {
  if (!validator.includes(marker)) fail(`packet validator core missing strict guard ${marker}`);
}
if (validator.includes('baselineReadWorkloads')) {
  fail('packet validator core must not restore first-candidate read parity');
}

for (const marker of [
  "const resultDigestContract = 'ordered_length_prefixed_json_v1'",
  'const readOrderMarkers = new Map',
  'requireReadOrdering',
  'validateReadEvidence',
  'validateMutationEvidence',
  'requirePlan',
  'validateDatabase',
  'validateDataset',
  'validateProvenance',
  'validateSourceOracle',
  'validateReadReport',
  'validateMutationReport',
  'validateMaintenanceReport',
  'same_result_digest_contract',
  'same_dataset_shape',
  'same_source_oracle_shape',
  'result_rows_ratio_1m_to_100k',
  'fail closed on report shape, metrics, plans, effects, ordering, digest semantics, and cardinalities',
  'Result digest contract:',
  '### Source oracle',
]) {
  if (!comparator.includes(marker)) fail(`evidence comparator core missing contract guard ${marker}`);
}
for (const legacy of [
  'values.filter(Number.isFinite)',
  'const numbers = (values)',
  'baselineReadWorkloads',
]) {
  if (comparator.includes(legacy)) fail(`evidence comparator core restored lossy validation: ${legacy}`);
}

for (const marker of [
  "test('rejects missing read digest contract'",
  "test('rejects provenance digest contract drift'",
  "test('rejects source workload without canonical ordering'",
  "test('rejects candidate workload without canonical ordering'",
  "test('rejects missing read execution timing'",
  "test('rejects malformed EXPLAIN plan'",
  "test('rejects missing mutation WAL metric'",
  "test('rejects candidate result drift from source oracle'",
  "test('rejects maintenance EAV field cardinality drift'",
  "test('rejects report repetition drift'",
  "test('rejects cross-scale commit mismatch'",
]) {
  if (!comparatorFixture.includes(marker)) {
    fail(`evidence comparator fixture coverage missing ${marker}`);
  }
}

for (const marker of [
  'const maskSqlText = (text) =>',
  'const executableSqlText = (sql, label) =>',
  "sql.startsWith('--', index)",
  "sql.startsWith('/*', index)",
  'unterminated block comment',
  'contains an unterminated ${kind}',
  'unterminated dollar-quoted string',
  'executableSql.trimEnd().endsWith(marker)',
  'must end with canonical ordering marker',
  'in executable SQL',
  'validatePacketReadOrdering',
  'source workload order',
  'prototype order',
]) {
  if (!orderingPreflight.includes(marker)) fail(`terminal ordering preflight missing ${marker}`);
}
for (const forbidden of ['sql.includes(marker)', 'sql.trimEnd().endsWith(marker)']) {
  if (orderingPreflight.includes(forbidden)) {
    fail(`terminal ordering preflight restored unsafe raw-SQL validation: ${forbidden}`);
  }
}
for (const marker of [
  "test('accepts canonical terminal ordering with trailing whitespace'",
  "test('accepts comment tokens inside strings and comments after executable ordering'",
  "test('rejects a source ordering marker that exists only in a nested query'",
  "test('rejects a candidate ordering marker that exists only in a block comment'",
  "test('rejects a terminal ordering marker hidden in a line comment'",
  "test('rejects an ordering marker hidden in a dollar-quoted string'",
  "test('rejects unterminated SQL comments before ordering validation'",
  "test('rejects workload order drift before checking SQL text'",
]) {
  if (!orderingFixture.includes(marker)) fail(`terminal ordering fixture coverage missing ${marker}`);
}
for (const marker of [
  "const preflight = read('scripts/verify/check-index-storage-read-ordering.mjs')",
  "const fixture = read('scripts/verify/check-index-storage-read-ordering.test.mjs')",
  "const standaloneGuard = read('scripts/verify/verify-index-storage-standalone-tools.mjs')",
  'executableSql.trimEnd().endsWith(marker)',
  "test('packet runs terminal ordering preflight before the canonical validator'",
  "test('compare runs terminal ordering preflight before the canonical comparator'",
  'standalone validator must revoke stale provenance, preflight ordering/resources, then run its isolated core',
  'standalone comparator must revoke stale JSON, preflight every input, then run its atomic lifecycle',
  'packet terminal ordering preflight must run before the canonical validator',
  'comparison terminal ordering preflight must run before the canonical comparator',
  'scripts/verify/verify-index-storage-read-ordering-contract.mjs',
]) {
  if (!orderingGuard.includes(marker)) fail(`terminal ordering guard missing ${marker}`);
}

const requireLifecycleOrder = (content, label, markers) => {
  let previous = -1;
  for (const marker of markers) {
    const index = content.indexOf(marker);
    if (index < 0) fail(`${label} missing lifecycle marker ${marker}`);
    if (index <= previous) fail(`${label} lifecycle marker is out of order: ${marker}`);
    previous = index;
  }
};

for (const [wrapper, label, markers, orderedMarkers, forbiddenMarkers] of [
  [validatorWrapper, 'validator wrapper', [
    "import { validatePacketReadOrdering } from './check-index-storage-read-ordering.mjs'",
    "import { runValidatorCoreWithAtomicProvenance } from './run-index-storage-validator-core.mjs'",
    "import { validateRunnerResourceSnapshots } from './validate-index-storage-runner-resources.mjs'",
    "const corePath = fileURLToPath(new URL('./validate-index-storage-evidence-core.mjs', import.meta.url))",
    'runValidatorCoreWithAtomicProvenance({ evidenceRoot, corePath })',
  ], [
    'invalidatePacketProvenance(evidenceRoot);',
    'validatePacketReadOrdering(evidenceRoot);',
    'validateRunnerResourceSnapshots(evidenceRoot);',
    'runValidatorCoreWithAtomicProvenance({ evidenceRoot, corePath })',
  ], [
    "await import('./validate-index-storage-evidence-core.mjs')",
  ]],
  [comparatorWrapper, 'comparator wrapper', [
    "import { validatePacketReadOrdering } from './check-index-storage-read-ordering.mjs'",
    "const corePath = fileURLToPath(new URL('./compare-index-storage-evidence-core.mjs', import.meta.url))",
    'runComparatorCoreWithAtomicComparison(parsed)',
  ], [
    "rmSync(path.join(parsed.output, 'comparison.json'), { force: true });",
    'for (const input of parsed.inputs) validatePacketReadOrdering(input);',
    'runComparatorCoreWithAtomicComparison(parsed)',
  ], [
    "await import('./compare-index-storage-evidence-core.mjs')",
  ]],
]) {
  for (const marker of markers) {
    if (!wrapper.includes(marker)) fail(`${label} missing strict entrypoint marker ${marker}`);
  }
  requireLifecycleOrder(wrapper, label, orderedMarkers);
  for (const marker of forbiddenMarkers) {
    if (wrapper.includes(marker)) fail(`${label} restored obsolete direct core import ${marker}`);
  }
  if (wrapper.includes('migrated to inspect the preserved core directly')) {
    fail(`${label} still contains the temporary compatibility marker shim`);
  }
}
for (const marker of [
  "const gitBlobSha = (bytes) => createHash('sha1')",
  "'dabc18d59360c300352ab3afb2510f0a0ff22796'",
  "'97ef0e8a216735e457c4c827d975462b84b009b3'",
  'runValidatorCoreWithAtomicProvenance',
  'runComparatorCoreWithAtomicComparison',
  'has lifecycle markers out of order',
]) {
  if (!standaloneGuard.includes(marker)) fail(`standalone evidence guard missing ${marker}`);
}
for (const marker of [
  "test('direct validator rejects non-executable terminal ordering before its core'",
  "test('direct comparator rejects nested-only terminal ordering before its core'",
  "test('direct validator reaches the byte-preserved core after a valid preflight'",
  "test('direct comparator forwards help to the byte-preserved core'",
]) {
  if (!standaloneFixture.includes(marker)) fail(`standalone evidence fixture coverage missing ${marker}`);
}

for (const marker of [
  "'verify-index-storage-read-ordering-contract.mjs'",
  "'verify-index-storage-standalone-tools.mjs'",
  "scriptPath('index-storage-standalone-tools.test.mjs')",
  "runScript('check-index-storage-read-ordering.mjs', ['--input', packetRoot])",
  "runScript('check-index-storage-read-ordering.mjs', orderingArgs)",
  "'verify-index-storage-adr-integrity.mjs'",
  "case 'verify-adr':",
  "runScript('verify-index-storage-adr.mjs', args)",
]) {
  if (!toolingRouter.includes(marker)) fail(`storage tooling router missing independent integrity wiring ${marker}`);
}
for (const marker of [
  'console.error(`[verify-index-storage-adr-integrity] ${message}`)',
  "const orderingGuard = read('scripts/verify/verify-index-storage-read-ordering-contract.mjs')",
  "const standaloneGuard = read('scripts/verify/verify-index-storage-standalone-tools.mjs')",
  'has lifecycle markers out of order',
  "runScript('finalize-index-storage-adr.mjs', args)",
  "runScript('verify-index-storage-adr.mjs', args)",
  'ADR bytes differ from deterministic finalization',
  'scripts/verify/verify-index-storage-adr-integrity.mjs',
]) {
  if (!adrIntegrityGuard.includes(marker)) fail(`ADR integrity guard missing cross-protection marker ${marker}`);
}
for (const marker of [
  'scripts/verify/check-index-storage-read-ordering.mjs',
  'scripts/verify/check-index-storage-read-ordering.test.mjs',
  'scripts/verify/verify-index-storage-read-ordering-contract.mjs',
  'scripts/verify/validate-index-storage-evidence-core.mjs',
  'scripts/verify/compare-index-storage-evidence-core.mjs',
  'scripts/verify/index-storage-standalone-tools.test.mjs',
  'scripts/verify/verify-index-storage-standalone-tools.mjs',
  'node --check scripts/verify/check-index-storage-read-ordering.mjs',
  'node --check scripts/verify/verify-index-storage-adr-integrity.mjs',
]) {
  if (!smokeWorkflow.includes(marker)) fail(`smoke workflow missing integrity wiring ${marker}`);
}
for (const marker of [
  'scripts/verify/*index-storage*.mjs',
  'scripts/verify/storage-decision*.mjs',
  'scripts/verify/*methodology-envelope*.mjs',
  'find scripts/verify -maxdepth 1 -type f',
  'node scripts/verify/index-storage-tooling.mjs contract',
  'node --test scripts/verify/index-storage-validator-arguments.test.mjs',
  'node scripts/verify/index-storage-tooling.mjs fixtures',
  "if: ${{ github.event_name == 'workflow_dispatch' }}",
]) {
  if (!scaleWorkflow.includes(marker)) fail(`scale workflow missing integrity wiring ${marker}`);
}

console.log('[verify-index-storage-source-oracle] source oracle, deterministic PostgreSQL string semantics, byte-preserved evidence cores, self-described ordered digests, complete metrics, executable SQL entrypoints, and ADR integrity wiring are independently guarded');
