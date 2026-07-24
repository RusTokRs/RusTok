import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const fail = (message) => {
  console.error(`[validate-index-storage-evidence] ${message}`);
  process.exit(1);
};

const resultDigestContract = 'ordered_length_prefixed_json_v1';
const canonicalLocales = ['en-US', 'ru-RU'];
const canonicalPrototypes = [
  { prototype: 'jsonb', schema: 'idx_bench_jsonb', relations: ['entity', 'link'] },
  { prototype: 'typed_eav', schema: 'idx_bench_eav', relations: ['entity', 'field_value', 'link'] },
  {
    prototype: 'hot_projection',
    schema: 'idx_bench_hot',
    relations: ['link', 'product', 'sales_channel', 'variant'],
  },
];
const canonicalReadWorkloads = [
  'status_equality',
  'price_range_sort',
  'multi_value_tag',
  'two_hop_channel_filter',
  'keyset_page',
  'exact_count',
];
const canonicalMutationWorkloads = ['update_product_batch', 'delete_product_batch'];
const readOrderMarkers = new Map([
  ['status_equality', 'ORDER BY entity_id LIMIT 100'],
  ['price_range_sort', 'ORDER BY price_minor, entity_id LIMIT 100'],
  ['multi_value_tag', 'ORDER BY entity_id LIMIT 100'],
  ['two_hop_channel_filter', 'ORDER BY entity_id LIMIT 100'],
  ['keyset_page', 'ORDER BY price_minor, entity_id LIMIT 100'],
  ['exact_count', null],
]);

const contracts = {
  smoke: {
    serializedScale: 'smoke',
    debugScale: 'Smoke',
    tenants: 2,
    productsPerTenant: 100,
    productRows: 400,
    entityRows: 1_216,
    eavFieldRows: 5_632,
    linkRows: 2_400,
    mutationBatch: 100,
    deletedLinks: 200,
  },
  '100k': {
    serializedScale: 'rows100k',
    debugScale: 'Rows100k',
    tenants: 10,
    productsPerTenant: 5_000,
    productRows: 100_000,
    entityRows: 300_080,
    eavFieldRows: 1_400_160,
    linkRows: 600_000,
    mutationBatch: 1_000,
    deletedLinks: 2_000,
  },
  '1m': {
    serializedScale: 'rows1m',
    debugScale: 'Rows1m',
    tenants: 20,
    productsPerTenant: 25_000,
    productRows: 1_000_000,
    entityRows: 3_000_160,
    eavFieldRows: 14_000_320,
    linkRows: 6_000_000,
    mutationBatch: 1_000,
    deletedLinks: 2_000,
  },
};

const scale = process.env.INDEX_BENCH_SCALE;
const contract = contracts[scale];
if (!contract) fail(`INDEX_BENCH_SCALE must be smoke, 100k, or 1m; got ${scale}`);

const root = process.env.INDEX_BENCH_EVIDENCE_ROOT
  ?? path.join('evidence/index-storage', scale);
const files = ['read-report.json', 'mutation-report.json', 'maintenance-report.json'];

const readJson = (filename) => {
  const fullPath = path.join(root, filename);
  if (!existsSync(fullPath)) fail(`missing evidence file: ${fullPath}`);
  try {
    return JSON.parse(readFileSync(fullPath, 'utf8'));
  } catch (error) {
    fail(`invalid JSON in ${fullPath}: ${error.message}`);
  }
};

const sameJson = (left, right) => JSON.stringify(left) === JSON.stringify(right);

const requireObject = (value, label) => {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    fail(`${label} must be an object`);
  }
  return value;
};

const requireNonEmptyString = (value, label) => {
  if (typeof value !== 'string' || value.length === 0) {
    fail(`${label} must be a non-empty string`);
  }
};

const requireTimestamp = (value, label) => {
  requireNonEmptyString(value, label);
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/u.test(value)
      || !Number.isFinite(Date.parse(value))) {
    fail(`${label} must be an RFC 3339 UTC timestamp`);
  }
};

const requireNonNegativeNumber = (value, label) => {
  if (!Number.isFinite(value) || value < 0) fail(`${label} must be a non-negative number`);
};

const requirePositiveInteger = (value, label) => {
  if (!Number.isInteger(value) || value <= 0) fail(`${label} must be a positive integer`);
};

const requireNonNegativeInteger = (value, label) => {
  if (!Number.isInteger(value) || value < 0) fail(`${label} must be a non-negative integer`);
};

const requireNullableNonNegativeInteger = (value, label) => {
  if (value !== null) requireNonNegativeInteger(value, label);
};

const requireDigest = (value, label) => {
  if (typeof value !== 'string' || !/^[0-9a-f]{32}$/u.test(value)) {
    fail(`${label} must be an MD5 digest`);
  }
};

const requireExactOrder = (items, expected, label) => {
  if (!Array.isArray(items)) fail(`${label} must be an array`);
  if (new Set(items).size !== items.length) fail(`${label} contains duplicate entries`);
  if (!sameJson(items, expected)) {
    fail(`${label} mismatch: expected ${expected.join(', ')}, got ${items.join(', ')}`);
  }
};

const requireReadOrdering = (sql, workloadName, label) => {
  if (!readOrderMarkers.has(workloadName)) fail(`${label} has no canonical ordering contract`);
  const marker = readOrderMarkers.get(workloadName);
  if (marker !== null && !sql.includes(marker)) {
    fail(`${label}.sql is missing canonical ordering marker ${marker}`);
  }
};

const requirePrototypeContract = (report, label) => {
  if (!Array.isArray(report.prototypes)) fail(`${label}.prototypes must be an array`);
  requireExactOrder(
    report.prototypes.map((prototype) => prototype?.prototype),
    canonicalPrototypes.map((prototype) => prototype.prototype),
    `${label} prototype order`,
  );
  report.prototypes.forEach((prototype, index) => {
    requireObject(prototype, `${label} prototype ${index}`);
    const expected = canonicalPrototypes[index];
    if (prototype.schema !== expected.schema) {
      fail(`${label}/${expected.prototype} schema mismatch: expected ${expected.schema}, got ${prototype.schema}`);
    }
  });
};

const requirePlan = (plan, label) => {
  if (!Array.isArray(plan) || plan.length !== 1
      || !plan[0] || typeof plan[0] !== 'object' || Array.isArray(plan[0])
      || !plan[0].Plan || typeof plan[0].Plan !== 'object' || Array.isArray(plan[0].Plan)) {
    fail(`${label} must contain one EXPLAIN JSON plan`);
  }
};

const requireReadExplain = (evidence, label) => {
  requireObject(evidence, label);
  requireNonNegativeNumber(evidence.planning_time_ms, `${label}.planning_time_ms`);
  requireNonNegativeNumber(evidence.execution_time_ms, `${label}.execution_time_ms`);
  requireNonNegativeInteger(evidence.shared_hit_blocks, `${label}.shared_hit_blocks`);
  requireNonNegativeInteger(evidence.shared_read_blocks, `${label}.shared_read_blocks`);
  requireNullableNonNegativeInteger(evidence.temporary_read_blocks, `${label}.temporary_read_blocks`);
  requireNullableNonNegativeInteger(evidence.temporary_written_blocks, `${label}.temporary_written_blocks`);
  requirePlan(evidence.plan, `${label}.plan`);
};

const requireMutationExplain = (evidence, label) => {
  requireReadExplain(evidence, label);
  for (const field of [
    'maximum_node_wal_records',
    'maximum_node_wal_fpi',
    'maximum_node_wal_bytes',
  ]) {
    requireNonNegativeInteger(evidence[field], `${label}.${field}`);
  }
};

const requireDatabaseContract = (database) => {
  requireObject(database, 'read.database');
  for (const field of [
    'version',
    'server_version_num',
    'shared_buffers',
    'effective_cache_size',
    'work_mem',
    'random_page_cost',
    'jit',
  ]) {
    requireNonEmptyString(database[field], `read.database.${field}`);
  }
  if (!/^\d+$/u.test(database.server_version_num)) {
    fail(`read.database.server_version_num must contain only digits; got ${database.server_version_num}`);
  }
  const serverVersion = Number.parseInt(database.server_version_num, 10);
  if (Math.floor(serverVersion / 10_000) !== 16) {
    fail(`read.database.server_version_num must describe PostgreSQL 16; got ${database.server_version_num}`);
  }
  if (database.jit !== 'off') fail(`read.database.jit must be off; got ${database.jit}`);
};

const read = readJson('read-report.json');
requireObject(read, 'read report');
requireTimestamp(read.generated_at, 'read.generated_at');
if (read.result_digest_contract !== resultDigestContract) {
  fail(`read.result_digest_contract must be ${resultDigestContract}; got ${read.result_digest_contract}`);
}
requireDatabaseContract(read.database);
const dataset = requireObject(read.dataset, 'read.dataset');
if (dataset.scale !== contract.serializedScale) {
  fail(`read scale mismatch: expected ${contract.serializedScale}, got ${dataset.scale}`);
}
if (dataset.tenants !== contract.tenants) {
  fail(`read tenant count mismatch: expected ${contract.tenants}, got ${dataset.tenants}`);
}
if (dataset.products_per_tenant !== contract.productsPerTenant) {
  fail(`read products-per-tenant mismatch: expected ${contract.productsPerTenant}, got ${dataset.products_per_tenant}`);
}
requireExactOrder(dataset.locales, canonicalLocales, 'read locale order');
if (dataset.variants_per_product !== 2 || dataset.channels_per_tenant !== 8
    || dataset.sales_channels_per_variant !== 2) {
  fail('read dataset topology mismatch');
}
const productRows = dataset.tenants * dataset.products_per_tenant * dataset.locales.length;
if (productRows !== contract.productRows) {
  fail(`read product-row mismatch: expected ${contract.productRows}, got ${productRows}`);
}
requireNonNegativeNumber(read.source_load_ms, 'read.source_load_ms');
if (read.source_entity_rows !== contract.entityRows || read.source_link_rows !== contract.linkRows) {
  fail('read source cardinality mismatch');
}

requireExactOrder(
  read.source_workloads?.map((workload) => workload?.name),
  canonicalReadWorkloads,
  'source workload order',
);
const sourceOracle = new Map();
for (const sourceWorkload of read.source_workloads) {
  requireObject(sourceWorkload, `source/${sourceWorkload?.name ?? 'unknown'}`);
  requireNonEmptyString(sourceWorkload.sql, `source/${sourceWorkload.name}.sql`);
  if (!sourceWorkload.sql.includes('idx_bench_source.')) {
    fail(`source/${sourceWorkload.name}.sql must read from idx_bench_source`);
  }
  requireReadOrdering(sourceWorkload.sql, sourceWorkload.name, `source/${sourceWorkload.name}`);
  requireNonNegativeInteger(sourceWorkload.result_rows, `source/${sourceWorkload.name}.result_rows`);
  requireDigest(sourceWorkload.result_digest, `source/${sourceWorkload.name}.result_digest`);
  sourceOracle.set(sourceWorkload.name, {
    resultRows: sourceWorkload.result_rows,
    resultDigest: sourceWorkload.result_digest,
  });
}

requirePrototypeContract(read, 'read report');
for (const [prototypeIndex, prototype] of read.prototypes.entries()) {
  const expectedPrototype = canonicalPrototypes[prototypeIndex];
  requireNonNegativeNumber(prototype.load_ms, `${expectedPrototype.prototype}.load_ms`);
  requirePositiveInteger(prototype.schema_bytes, `${expectedPrototype.prototype}.schema_bytes`);
  if (prototype.entity_rows !== contract.entityRows || prototype.link_rows !== contract.linkRows) {
    fail(`${prototype.prototype} read cardinality mismatch`);
  }
  requireExactOrder(
    prototype.workloads?.map((workload) => workload?.name),
    canonicalReadWorkloads,
    `${prototype.prototype} read workload order`,
  );
  for (const workload of prototype.workloads) {
    requireNonEmptyString(workload.sql, `${prototype.prototype}/${workload.name}.sql`);
    if (workload.sql.includes('idx_bench_source.')) {
      fail(`${prototype.prototype}/${workload.name}.sql must not read from source oracle tables`);
    }
    requireReadOrdering(workload.sql, workload.name, `${prototype.prototype}/${workload.name}`);
    requireNonNegativeInteger(workload.result_rows, `${prototype.prototype}/${workload.name}.result_rows`);
    requireDigest(workload.result_digest, `${prototype.prototype}/${workload.name}.result_digest`);
    if (!Array.isArray(workload.repetitions) || workload.repetitions.length !== 3) {
      fail(`${prototype.prototype}/${workload.name} read repetitions mismatch`);
    }
    workload.repetitions.forEach((evidence, repetition) => {
      requireReadExplain(evidence, `${prototype.prototype}/${workload.name}/repetition-${repetition + 1}`);
    });

    const expected = sourceOracle.get(workload.name);
    if (!expected
        || expected.resultRows !== workload.result_rows
        || expected.resultDigest !== workload.result_digest) {
      fail(`${prototype.prototype}/${workload.name} differs from source oracle`);
    }
  }
}

const mutation = readJson('mutation-report.json');
requireObject(mutation, 'mutation report');
requireTimestamp(mutation.generated_at, 'mutation.generated_at');
if (mutation.dataset_scale !== contract.debugScale || mutation.repetitions !== 3) {
  fail('mutation scale/repetition mismatch');
}
requirePrototypeContract(mutation, 'mutation report');
for (const prototype of mutation.prototypes) {
  const eavFields = prototype.prototype === 'typed_eav';
  const expectedMutations = new Map([
    ['update_product_batch', {
      entities: contract.mutationBatch,
      fields: eavFields ? contract.mutationBatch * 2 : null,
      links: null,
    }],
    ['delete_product_batch', {
      entities: contract.mutationBatch,
      fields: eavFields ? contract.mutationBatch * 8 : null,
      links: contract.deletedLinks,
    }],
  ]);
  requireExactOrder(
    prototype.workloads?.map((workload) => workload?.name),
    canonicalMutationWorkloads,
    `${prototype.prototype} mutation workload order`,
  );
  for (const workload of prototype.workloads) {
    const expected = expectedMutations.get(workload.name);
    requireNonEmptyString(workload.sql, `${prototype.prototype}/${workload.name}.sql`);
    for (const marker of ['affected_fields', 'expected_fields', 'affected_links', 'expected_links']) {
      if (!workload.sql.includes(marker)) {
        fail(`${prototype.prototype}/${workload.name}.sql is missing ${marker}`);
      }
    }
    if (workload.affected_entities !== expected.entities
        || workload.affected_fields !== expected.fields
        || workload.affected_links !== expected.links) {
      fail(`${prototype.prototype}/${workload.name} mutation effect mismatch`);
    }
    if (!Array.isArray(workload.repetitions) || workload.repetitions.length !== 3) {
      fail(`${prototype.prototype}/${workload.name} mutation repetitions mismatch`);
    }
    workload.repetitions.forEach((evidence, repetition) => {
      requireMutationExplain(evidence, `${prototype.prototype}/${workload.name}/repetition-${repetition + 1}`);
    });
  }
}

const maintenance = readJson('maintenance-report.json');
requireObject(maintenance, 'maintenance report');
requireTimestamp(maintenance.generated_at, 'maintenance.generated_at');
if (maintenance.dataset_scale !== contract.serializedScale || maintenance.cycles !== 5) {
  fail('maintenance scale/cycle mismatch');
}
requirePrototypeContract(maintenance, 'maintenance report');
const statFields = [
  'estimated_live_tuples',
  'estimated_dead_tuples',
  'tuples_inserted',
  'tuples_updated',
  'tuples_deleted',
  'hot_updates',
  'vacuum_count',
  'autovacuum_count',
  'analyze_count',
  'autoanalyze_count',
];
for (const [prototypeIndex, prototype] of maintenance.prototypes.entries()) {
  const expectedPrototype = canonicalPrototypes[prototypeIndex];
  const expectedFieldRows = prototype.prototype === 'typed_eav' ? contract.eavFieldRows : null;
  for (const phase of ['baseline', 'after_churn', 'after_vacuum']) {
    const snapshot = requireObject(prototype[phase], `${prototype.prototype}/${phase}`);
    requireTimestamp(snapshot.captured_at, `${prototype.prototype}/${phase}.captured_at`);
    requirePositiveInteger(snapshot.schema_bytes, `${prototype.prototype}/${phase}.schema_bytes`);
    if (snapshot.entity_rows !== contract.entityRows
        || snapshot.field_rows !== expectedFieldRows
        || snapshot.link_rows !== contract.linkRows) {
      fail(`${prototype.prototype}/${phase} maintenance cardinality mismatch`);
    }
    requireExactOrder(
      snapshot.table_stats?.map((stats) => stats?.relation),
      expectedPrototype.relations,
      `${prototype.prototype}/${phase} relation order`,
    );
    for (const stats of snapshot.table_stats) {
      requireObject(stats, `${prototype.prototype}/${phase}/${stats?.relation ?? 'unknown'}`);
      for (const field of statFields) {
        requireNonNegativeInteger(stats[field], `${prototype.prototype}/${phase}/${stats.relation}.${field}`);
      }
    }
  }
  requireNonNegativeNumber(prototype.vacuum_duration_ms, `${prototype.prototype}.vacuum_duration_ms`);
}

const resourceFiles = [
  'runner-resources-before.txt',
  'runner-resources-after.txt',
].filter((filename) => existsSync(path.join(root, filename)));
if (process.env.INDEX_BENCH_REQUIRE_RUNNER_RESOURCES === '1' && resourceFiles.length !== 2) {
  fail('scale evidence must include before/after runner resource snapshots');
}

const githubProvenance = {
  repository: process.env.GITHUB_REPOSITORY ?? null,
  commit: process.env.GITHUB_SHA ?? null,
  ref: process.env.GITHUB_REF ?? null,
  run_id: process.env.GITHUB_RUN_ID ?? null,
  run_attempt: process.env.GITHUB_RUN_ATTEMPT ?? null,
  job: process.env.GITHUB_JOB ?? null,
  runner_os: process.env.RUNNER_OS ?? null,
  runner_arch: process.env.RUNNER_ARCH ?? null,
};
if (process.env.INDEX_BENCH_REQUIRE_GITHUB_PROVENANCE === '1') {
  for (const [field, value] of Object.entries(githubProvenance)) {
    requireNonEmptyString(value, `GitHub provenance ${field}`);
  }
  if (!/^[0-9a-f]{40}$/iu.test(githubProvenance.commit)) {
    fail(`GitHub provenance commit must be a full SHA; got ${githubProvenance.commit}`);
  }
  if (!/^\d+$/u.test(githubProvenance.run_id) || !/^\d+$/u.test(githubProvenance.run_attempt)) {
    fail('GitHub provenance run_id and run_attempt must be numeric strings');
  }
}

writeFileSync(
  path.join(root, 'provenance.json'),
  JSON.stringify({
    packet_contract_version: 2,
    generated_at: new Date().toISOString(),
    ...githubProvenance,
    postgres_image: 'postgres:16',
    scale,
    repetitions: 3,
    churn_cycles: 5,
    result_digest_contract: resultDigestContract,
    source_workload_names: canonicalReadWorkloads,
    expected_product_rows: contract.productRows,
    expected_entity_rows: contract.entityRows,
    expected_eav_field_rows: contract.eavFieldRows,
    expected_link_rows: contract.linkRows,
    reports: files,
    runner_resource_files: resourceFiles,
  }, null, 2) + '\n',
);

console.log(
  `[validate-index-storage-evidence] ${scale} packet is consistent: `
  + `${contract.entityRows} entities, ${contract.eavFieldRows} EAV fields, ${contract.linkRows} links`,
);
