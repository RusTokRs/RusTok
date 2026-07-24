import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const die = (message) => {
  console.error(`[compare-index-storage-evidence] ${message}`);
  process.exit(1);
};

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
const comparableDatabaseFields = [
  'server_version_num',
  'shared_buffers',
  'effective_cache_size',
  'work_mem',
  'random_page_cost',
  'jit',
];
const requiredDatabaseFields = ['version', ...comparableDatabaseFields];
const maintenanceStatFields = [
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

const parseArgs = () => {
  const inputs = [];
  let output = 'evidence/index-storage/comparison';
  const args = process.argv.slice(2);
  for (let index = 0; index < args.length; index += 1) {
    if (args[index] === '--input' && args[index + 1]) inputs.push(args[++index]);
    else if (args[index] === '--output' && args[index + 1]) output = args[++index];
    else if (args[index] === '--help' || args[index] === '-h') {
      console.log('Usage: node scripts/verify/compare-index-storage-evidence.mjs --input <dir> [--input <dir>] [--output <dir>]');
      process.exit(0);
    } else die(`unknown or incomplete argument: ${args[index]}`);
  }
  if (inputs.length === 0) die('at least one --input directory is required');
  return { inputs, output };
};

const readJson = (directory, filename) => {
  const file = path.join(directory, filename);
  if (!existsSync(file)) die(`missing evidence file: ${file}`);
  try {
    return JSON.parse(readFileSync(file, 'utf8'));
  } catch (error) {
    die(`invalid JSON in ${file}: ${error.message}`);
  }
};

const sameJson = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const requireObject = (value, label) => {
  if (!value || typeof value !== 'object' || Array.isArray(value)) die(`${label} must be an object`);
  return value;
};
const requireArray = (value, label) => {
  if (!Array.isArray(value)) die(`${label} must be an array`);
  return value;
};
const requireNonEmptyString = (value, label) => {
  if (typeof value !== 'string' || value.length === 0) die(`${label} must be a non-empty string`);
};
const requireTimestamp = (value, label) => {
  requireNonEmptyString(value, label);
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/u.test(value)
      || !Number.isFinite(Date.parse(value))) {
    die(`${label} must be an RFC 3339 UTC timestamp`);
  }
};
const requireNonNegativeNumber = (value, label) => {
  if (!Number.isFinite(value) || value < 0) die(`${label} must be a non-negative number`);
};
const requirePositiveInteger = (value, label) => {
  if (!Number.isInteger(value) || value <= 0) die(`${label} must be a positive integer`);
};
const requireNonNegativeInteger = (value, label) => {
  if (!Number.isInteger(value) || value < 0) die(`${label} must be a non-negative integer`);
};
const requireNullableNonNegativeInteger = (value, label) => {
  if (value !== null) requireNonNegativeInteger(value, label);
};
const requireDigest = (value, label) => {
  if (typeof value !== 'string' || !/^[0-9a-f]{32}$/u.test(value)) die(`${label} must be an MD5 digest`);
};
const requireExactOrder = (actual, expected, label) => {
  requireArray(actual, label);
  if (new Set(actual).size !== actual.length) die(`${label} contains duplicate entries`);
  if (!sameJson(actual, expected)) {
    die(`${label} mismatch: expected ${expected.join(', ')}, got ${actual.join(', ')}`);
  }
};
const requirePlan = (plan, label) => {
  if (!Array.isArray(plan) || plan.length !== 1 || !plan[0] || typeof plan[0] !== 'object'
      || !plan[0].Plan || typeof plan[0].Plan !== 'object') {
    die(`${label} must contain one EXPLAIN JSON plan`);
  }
};

const median = (values) => {
  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 1
    ? sorted[middle]
    : (sorted[middle - 1] + sorted[middle]) / 2;
};
const maximum = (values) => Math.max(...values);
const ratio = (numerator, denominator) => denominator === 0 ? null : numerator / denominator;
const percent = (delta, base) => {
  const value = ratio(delta, base);
  return value === null ? null : value * 100;
};
const planShape = (plan) => {
  const walk = (node) => !node || typeof node !== 'object' ? null : ({
    node: node['Node Type'] ?? null,
    relation: node['Relation Name'] ?? null,
    index: node['Index Name'] ?? null,
    join: node['Join Type'] ?? null,
    strategy: node.Strategy ?? null,
    plans: Array.isArray(node.Plans) ? node.Plans.map(walk) : [],
  });
  return JSON.stringify(walk(plan[0].Plan));
};

const validateReadEvidence = (evidence, label) => {
  requireObject(evidence, label);
  requireNonNegativeNumber(evidence.planning_time_ms, `${label}.planning_time_ms`);
  requireNonNegativeNumber(evidence.execution_time_ms, `${label}.execution_time_ms`);
  requireNonNegativeInteger(evidence.shared_hit_blocks, `${label}.shared_hit_blocks`);
  requireNonNegativeInteger(evidence.shared_read_blocks, `${label}.shared_read_blocks`);
  requireNullableNonNegativeInteger(evidence.temporary_read_blocks, `${label}.temporary_read_blocks`);
  requireNullableNonNegativeInteger(evidence.temporary_written_blocks, `${label}.temporary_written_blocks`);
  requirePlan(evidence.plan, `${label}.plan`);
};
const validateMutationEvidence = (evidence, label) => {
  validateReadEvidence(evidence, label);
  requireNonNegativeInteger(evidence.maximum_node_wal_records, `${label}.maximum_node_wal_records`);
  requireNonNegativeInteger(evidence.maximum_node_wal_fpi, `${label}.maximum_node_wal_fpi`);
  requireNonNegativeInteger(evidence.maximum_node_wal_bytes, `${label}.maximum_node_wal_bytes`);
};
const summarizeExplain = (repetitions) => {
  const warm = repetitions.slice(1);
  const warmEvidence = warm.length > 0 ? warm : repetitions;
  return {
    repetitions: repetitions.length,
    first_execution_ms: repetitions[0].execution_time_ms,
    warm_median_execution_ms: median(warmEvidence.map((item) => item.execution_time_ms)),
    median_execution_ms: median(repetitions.map((item) => item.execution_time_ms)),
    median_planning_ms: median(repetitions.map((item) => item.planning_time_ms)),
    first_shared_read_blocks: repetitions[0].shared_read_blocks,
    warm_median_shared_read_blocks: median(warmEvidence.map((item) => item.shared_read_blocks)),
    warm_median_shared_hit_blocks: median(warmEvidence.map((item) => item.shared_hit_blocks)),
    median_temp_read_blocks: median(repetitions.map((item) => item.temporary_read_blocks ?? 0)),
    median_temp_written_blocks: median(repetitions.map((item) => item.temporary_written_blocks ?? 0)),
    plan_shape_variants: new Set(repetitions.map((item) => planShape(item.plan))).size,
  };
};

const validateDatabase = (database, scale) => {
  requireObject(database, `${scale} database`);
  for (const field of requiredDatabaseFields) {
    requireNonEmptyString(database[field], `${scale} database.${field}`);
  }
  if (!/^\d+$/u.test(database.server_version_num)) {
    die(`${scale} database.server_version_num must contain only digits`);
  }
  const serverVersion = Number.parseInt(database.server_version_num, 10);
  if (Math.floor(serverVersion / 10_000) !== 16) {
    die(`${scale} database.server_version_num must describe PostgreSQL 16`);
  }
  if (database.jit !== 'off') die(`${scale} database.jit must be off`);
};

const validateDataset = (dataset, contract, scale) => {
  requireObject(dataset, `${scale} dataset`);
  if (dataset.scale !== contract.serializedScale
      || dataset.tenants !== contract.tenants
      || dataset.products_per_tenant !== contract.productsPerTenant
      || !sameJson(dataset.locales, canonicalLocales)
      || dataset.variants_per_product !== 2
      || dataset.channels_per_tenant !== 8
      || dataset.sales_channels_per_variant !== 2) {
    die(`${scale} dataset does not match the canonical scale contract`);
  }
  return dataset;
};

const validateProvenance = (directory, provenance, contract, scale) => {
  requireObject(provenance, `${scale} provenance`);
  if (provenance.packet_contract_version !== 2) {
    die(`${scale} evidence must use packet contract version 2`);
  }
  requireTimestamp(provenance.generated_at, `${scale} provenance.generated_at`);
  if (provenance.scale !== scale || provenance.postgres_image !== 'postgres:16') {
    die(`${scale} provenance scale/PostgreSQL image mismatch`);
  }
  if (provenance.repetitions !== 3 || provenance.churn_cycles !== 5) {
    die(`${scale} provenance must use 3 repetitions and 5 churn cycles`);
  }
  requireExactOrder(
    provenance.source_workload_names,
    canonicalReadWorkloads,
    `${scale} provenance source workload order`,
  );
  const expected = {
    expected_product_rows: contract.productRows,
    expected_entity_rows: contract.entityRows,
    expected_eav_field_rows: contract.eavFieldRows,
    expected_link_rows: contract.linkRows,
  };
  for (const [field, value] of Object.entries(expected)) {
    if (provenance[field] !== value) die(`${scale} provenance ${field} mismatch`);
  }
  requireExactOrder(
    provenance.reports,
    ['read-report.json', 'mutation-report.json', 'maintenance-report.json'],
    `${scale} provenance report order`,
  );

  if (scale !== 'smoke') {
    for (const field of ['repository', 'commit', 'ref', 'run_id', 'run_attempt', 'job', 'runner_os', 'runner_arch']) {
      requireNonEmptyString(provenance[field], `${scale} provenance.${field}`);
    }
    if (!/^[0-9a-f]{40}$/iu.test(provenance.commit)) {
      die(`${scale} provenance.commit must be a full SHA`);
    }
    if (!/^\d+$/u.test(provenance.run_id) || !/^\d+$/u.test(provenance.run_attempt)) {
      die(`${scale} provenance run identifiers must be numeric strings`);
    }
    requireExactOrder(
      provenance.runner_resource_files,
      ['runner-resources-before.txt', 'runner-resources-after.txt'],
      `${scale} provenance runner resource order`,
    );
    for (const filename of provenance.runner_resource_files) {
      if (!existsSync(path.join(directory, filename))) {
        die(`${scale} evidence is missing runner resource file ${filename}`);
      }
    }
  }
};

const validateSourceOracle = (read, contract, scale) => {
  if (read.source_entity_rows !== contract.entityRows || read.source_link_rows !== contract.linkRows) {
    die(`${scale} source cardinality mismatch`);
  }
  requireNonNegativeNumber(read.source_load_ms, `${scale} source_load_ms`);
  const sourceWorkloads = requireArray(read.source_workloads, `${scale} source_workloads`);
  requireExactOrder(
    sourceWorkloads.map((item) => item?.name),
    canonicalReadWorkloads,
    `${scale} source workload order`,
  );
  const oracle = new Map();
  for (const item of sourceWorkloads) {
    const label = `${scale} source/${item.name}`;
    requireObject(item, label);
    requireNonEmptyString(item.sql, `${label}.sql`);
    if (!item.sql.includes('idx_bench_source.')) die(`${label}.sql must read from idx_bench_source`);
    requireNonNegativeInteger(item.result_rows, `${label}.result_rows`);
    requireDigest(item.result_digest, `${label}.result_digest`);
    oracle.set(item.name, item);
  }
  return oracle;
};

const validateReadReport = (read, oracle, contract, scale) => {
  requireTimestamp(read.generated_at, `${scale} read.generated_at`);
  const prototypes = requireArray(read.prototypes, `${scale} read.prototypes`);
  requireExactOrder(
    prototypes.map((item) => item?.prototype),
    canonicalPrototypes.map((item) => item.prototype),
    `${scale} read prototype order`,
  );
  return prototypes.map((prototype, prototypeIndex) => {
    const expectedPrototype = canonicalPrototypes[prototypeIndex];
    const label = `${scale}/${expectedPrototype.prototype}`;
    requireObject(prototype, label);
    if (prototype.schema !== expectedPrototype.schema) die(`${label} schema mismatch`);
    requireNonNegativeNumber(prototype.load_ms, `${label}.load_ms`);
    requirePositiveInteger(prototype.schema_bytes, `${label}.schema_bytes`);
    if (prototype.entity_rows !== contract.entityRows || prototype.link_rows !== contract.linkRows) {
      die(`${label} read cardinality mismatch`);
    }
    const workloads = requireArray(prototype.workloads, `${label} workloads`);
    requireExactOrder(
      workloads.map((item) => item?.name),
      canonicalReadWorkloads,
      `${label} read workload order`,
    );
    return {
      prototype: prototype.prototype,
      schema: prototype.schema,
      load_ms: prototype.load_ms,
      schema_bytes: prototype.schema_bytes,
      entity_rows: prototype.entity_rows,
      link_rows: prototype.link_rows,
      workloads: workloads.map((workload) => {
        const workloadLabel = `${label}/${workload.name}`;
        requireObject(workload, workloadLabel);
        requireNonEmptyString(workload.sql, `${workloadLabel}.sql`);
        if (workload.sql.includes('idx_bench_source.')) {
          die(`${workloadLabel}.sql must not read from the source oracle tables`);
        }
        requireNonNegativeInteger(workload.result_rows, `${workloadLabel}.result_rows`);
        requireDigest(workload.result_digest, `${workloadLabel}.result_digest`);
        const expected = oracle.get(workload.name);
        if (!expected
            || workload.result_rows !== expected.result_rows
            || workload.result_digest !== expected.result_digest) {
          die(`${workloadLabel} differs from source oracle`);
        }
        const repetitions = requireArray(workload.repetitions, `${workloadLabel}.repetitions`);
        if (repetitions.length !== 3) die(`${workloadLabel} must contain 3 read repetitions`);
        repetitions.forEach((evidence, index) => {
          validateReadEvidence(evidence, `${workloadLabel}/repetition-${index + 1}`);
        });
        return {
          name: workload.name,
          result_rows: workload.result_rows,
          result_digest: workload.result_digest,
          ...summarizeExplain(repetitions),
        };
      }),
    };
  });
};

const mutationEffects = (prototype, workloadName, contract) => {
  const typedEav = prototype === 'typed_eav';
  if (workloadName === 'update_product_batch') {
    return {
      entities: contract.mutationBatch,
      fields: typedEav ? contract.mutationBatch * 2 : null,
      links: null,
    };
  }
  return {
    entities: contract.mutationBatch,
    fields: typedEav ? contract.mutationBatch * 8 : null,
    links: contract.deletedLinks,
  };
};

const validateMutationReport = (mutation, contract, scale) => {
  requireTimestamp(mutation.generated_at, `${scale} mutation.generated_at`);
  if (mutation.dataset_scale !== contract.debugScale || mutation.repetitions !== 3) {
    die(`${scale} mutation scale/repetition mismatch`);
  }
  const prototypes = requireArray(mutation.prototypes, `${scale} mutation.prototypes`);
  requireExactOrder(
    prototypes.map((item) => item?.prototype),
    canonicalPrototypes.map((item) => item.prototype),
    `${scale} mutation prototype order`,
  );
  return prototypes.map((prototype, prototypeIndex) => {
    const expectedPrototype = canonicalPrototypes[prototypeIndex];
    const label = `${scale}/${expectedPrototype.prototype}`;
    requireObject(prototype, `${label} mutation`);
    if (prototype.schema !== expectedPrototype.schema) die(`${label} mutation schema mismatch`);
    const workloads = requireArray(prototype.workloads, `${label} mutation workloads`);
    requireExactOrder(
      workloads.map((item) => item?.name),
      canonicalMutationWorkloads,
      `${label} mutation workload order`,
    );
    return {
      prototype: prototype.prototype,
      schema: prototype.schema,
      workloads: workloads.map((workload) => {
        const workloadLabel = `${label}/${workload.name}`;
        requireObject(workload, workloadLabel);
        requireNonEmptyString(workload.sql, `${workloadLabel}.sql`);
        for (const marker of ['affected_fields', 'expected_fields', 'affected_links', 'expected_links']) {
          if (!workload.sql.includes(marker)) die(`${workloadLabel}.sql is missing ${marker}`);
        }
        const expected = mutationEffects(prototype.prototype, workload.name, contract);
        if (workload.affected_entities !== expected.entities
            || workload.affected_fields !== expected.fields
            || workload.affected_links !== expected.links) {
          die(`${workloadLabel} mutation effect mismatch`);
        }
        const repetitions = requireArray(workload.repetitions, `${workloadLabel}.repetitions`);
        if (repetitions.length !== 3) die(`${workloadLabel} must contain 3 mutation repetitions`);
        repetitions.forEach((evidence, index) => {
          validateMutationEvidence(evidence, `${workloadLabel}/repetition-${index + 1}`);
        });
        return {
          name: workload.name,
          affected_entities: workload.affected_entities,
          affected_fields: workload.affected_fields,
          affected_links: workload.affected_links,
          ...summarizeExplain(repetitions),
          median_maximum_node_wal_records: median(repetitions.map((item) => item.maximum_node_wal_records)),
          median_maximum_node_wal_fpi: median(repetitions.map((item) => item.maximum_node_wal_fpi)),
          median_maximum_node_wal_bytes: median(repetitions.map((item) => item.maximum_node_wal_bytes)),
          peak_maximum_node_wal_bytes: maximum(repetitions.map((item) => item.maximum_node_wal_bytes)),
        };
      }),
    };
  });
};

const summarizeSnapshot = (snapshot) => ({
  schema_bytes: snapshot.schema_bytes,
  entity_rows: snapshot.entity_rows,
  field_rows: snapshot.field_rows,
  link_rows: snapshot.link_rows,
  estimated_live_tuples: snapshot.table_stats.reduce((total, item) => total + item.estimated_live_tuples, 0),
  estimated_dead_tuples: snapshot.table_stats.reduce((total, item) => total + item.estimated_dead_tuples, 0),
  tuples_inserted: snapshot.table_stats.reduce((total, item) => total + item.tuples_inserted, 0),
  tuples_updated: snapshot.table_stats.reduce((total, item) => total + item.tuples_updated, 0),
  tuples_deleted: snapshot.table_stats.reduce((total, item) => total + item.tuples_deleted, 0),
  hot_updates: snapshot.table_stats.reduce((total, item) => total + item.hot_updates, 0),
});

const validateMaintenanceSnapshot = (snapshot, prototype, phase, contract, scale) => {
  const label = `${scale}/${prototype.prototype}/${phase}`;
  requireObject(snapshot, label);
  requireTimestamp(snapshot.captured_at, `${label}.captured_at`);
  requirePositiveInteger(snapshot.schema_bytes, `${label}.schema_bytes`);
  const expectedFieldRows = prototype.prototype === 'typed_eav' ? contract.eavFieldRows : null;
  if (snapshot.entity_rows !== contract.entityRows
      || snapshot.field_rows !== expectedFieldRows
      || snapshot.link_rows !== contract.linkRows) {
    die(`${label} maintenance cardinality mismatch`);
  }
  const tableStats = requireArray(snapshot.table_stats, `${label}.table_stats`);
  requireExactOrder(
    tableStats.map((item) => item?.relation),
    prototype.relations,
    `${label} relation order`,
  );
  for (const stats of tableStats) {
    requireObject(stats, `${label}/${stats?.relation ?? 'unknown'}`);
    for (const field of maintenanceStatFields) {
      requireNonNegativeInteger(stats[field], `${label}/${stats.relation}.${field}`);
    }
  }
  return snapshot;
};

const validateMaintenanceReport = (maintenance, contract, scale) => {
  requireTimestamp(maintenance.generated_at, `${scale} maintenance.generated_at`);
  if (maintenance.dataset_scale !== contract.serializedScale || maintenance.cycles !== 5) {
    die(`${scale} maintenance scale/cycle mismatch`);
  }
  const prototypes = requireArray(maintenance.prototypes, `${scale} maintenance.prototypes`);
  requireExactOrder(
    prototypes.map((item) => item?.prototype),
    canonicalPrototypes.map((item) => item.prototype),
    `${scale} maintenance prototype order`,
  );
  return prototypes.map((item, prototypeIndex) => {
    const prototype = canonicalPrototypes[prototypeIndex];
    const label = `${scale}/${prototype.prototype}`;
    requireObject(item, `${label} maintenance`);
    if (item.schema !== prototype.schema) die(`${label} maintenance schema mismatch`);
    const baseline = summarizeSnapshot(validateMaintenanceSnapshot(item.baseline, prototype, 'baseline', contract, scale));
    const afterChurn = summarizeSnapshot(validateMaintenanceSnapshot(item.after_churn, prototype, 'after_churn', contract, scale));
    const afterVacuum = summarizeSnapshot(validateMaintenanceSnapshot(item.after_vacuum, prototype, 'after_vacuum', contract, scale));
    requireNonNegativeNumber(item.vacuum_duration_ms, `${label}.vacuum_duration_ms`);
    const vacuumSizeDelta = afterVacuum.schema_bytes - afterChurn.schema_bytes;
    return {
      prototype: item.prototype,
      schema: item.schema,
      baseline,
      after_churn: afterChurn,
      after_vacuum: afterVacuum,
      churn_growth_bytes: afterChurn.schema_bytes - baseline.schema_bytes,
      churn_growth_percent: percent(afterChurn.schema_bytes - baseline.schema_bytes, baseline.schema_bytes),
      vacuum_size_delta_bytes: vacuumSizeDelta,
      vacuum_size_delta_percent: percent(vacuumSizeDelta, afterChurn.schema_bytes),
      vacuum_duration_ms: item.vacuum_duration_ms,
    };
  });
};

const loadScale = (directory) => {
  const read = requireObject(readJson(directory, 'read-report.json'), `${directory}/read-report.json`);
  const mutation = requireObject(readJson(directory, 'mutation-report.json'), `${directory}/mutation-report.json`);
  const maintenance = requireObject(readJson(directory, 'maintenance-report.json'), `${directory}/maintenance-report.json`);
  const provenance = requireObject(readJson(directory, 'provenance.json'), `${directory}/provenance.json`);
  const scale = provenance.scale;
  const contract = contracts[scale];
  if (!contract) die(`unsupported evidence scale in ${directory}: ${scale}`);

  validateProvenance(directory, provenance, contract, scale);
  validateDatabase(read.database, scale);
  const dataset = validateDataset(read.dataset, contract, scale);
  if (mutation.dataset_scale !== contract.debugScale || maintenance.dataset_scale !== contract.serializedScale) {
    die(`${scale} report scale mismatch`);
  }
  const oracle = validateSourceOracle(read, contract, scale);
  const readSummary = validateReadReport(read, oracle, contract, scale);
  const mutationSummary = validateMutationReport(mutation, contract, scale);
  const maintenanceSummary = validateMaintenanceReport(maintenance, contract, scale);

  return {
    scale,
    directory,
    provenance: {
      packet_contract_version: provenance.packet_contract_version,
      repository: provenance.repository ?? null,
      commit: provenance.commit ?? null,
      ref: provenance.ref ?? null,
      run_id: provenance.run_id ?? null,
      run_attempt: provenance.run_attempt ?? null,
      postgres_image: provenance.postgres_image,
      runner_os: provenance.runner_os ?? null,
      runner_arch: provenance.runner_arch ?? null,
      repetitions: provenance.repetitions,
      churn_cycles: provenance.churn_cycles,
      source_workload_names: provenance.source_workload_names,
      expected_product_rows: provenance.expected_product_rows,
      expected_entity_rows: provenance.expected_entity_rows,
      expected_eav_field_rows: provenance.expected_eav_field_rows,
      expected_link_rows: provenance.expected_link_rows,
    },
    database: read.database,
    dataset,
    source_load_ms: read.source_load_ms,
    source_entity_rows: read.source_entity_rows,
    source_link_rows: read.source_link_rows,
    source_workloads: canonicalReadWorkloads.map((name) => {
      const item = oracle.get(name);
      return {
        name: item.name,
        sql: item.sql,
        result_rows: item.result_rows,
        result_digest: item.result_digest,
      };
    }),
    read: readSummary,
    mutation: mutationSummary,
    maintenance: maintenanceSummary,
  };
};

const candidate = (scale, section, name) => scale[section].find((item) => item.prototype === name);
const workload = (item, name) => item.workloads.find((entry) => entry.name === name);
const datasetShape = (dataset) => ({
  locales: dataset.locales,
  variants_per_product: dataset.variants_per_product,
  channels_per_tenant: dataset.channels_per_tenant,
  sales_channels_per_variant: dataset.sales_channels_per_variant,
});
const mutationEffectsOf = (items) => items.map((item) => ({
  prototype: item.prototype,
  workloads: item.workloads.map((entry) => ({
    name: entry.name,
    affected_entities: entry.affected_entities,
    affected_fields: entry.affected_fields,
    affected_links: entry.affected_links,
  })),
}));

const requireDecisionProvenance = (scales) => {
  const lower = scales.find((item) => item.scale === '100k');
  const upper = scales.find((item) => item.scale === '1m');
  if (!lower || !upper) {
    return {
      required_scales_present: false,
      same_packet_contract_version: null,
      same_repository: null,
      same_commit: null,
      same_postgres_image: null,
      same_repetitions: null,
      same_churn_cycles: null,
      same_database_settings: null,
      same_dataset_shape: null,
      same_source_oracle_shape: null,
      same_report_shape: null,
      same_mutation_effect_contract: null,
    };
  }
  const equal = (left, right, label) => {
    if (!sameJson(left, right)) die(`cross-scale ${label} mismatch`);
  };
  equal(lower.provenance.packet_contract_version, upper.provenance.packet_contract_version, 'packet contract version');
  equal(lower.provenance.repository, upper.provenance.repository, 'repository');
  equal(lower.provenance.commit, upper.provenance.commit, 'commit');
  equal(lower.provenance.postgres_image, upper.provenance.postgres_image, 'PostgreSQL image');
  equal(lower.provenance.repetitions, upper.provenance.repetitions, 'repetitions');
  equal(lower.provenance.churn_cycles, upper.provenance.churn_cycles, 'churn cycles');
  for (const field of comparableDatabaseFields) {
    equal(lower.database[field], upper.database[field], `database setting ${field}`);
  }
  equal(datasetShape(lower.dataset), datasetShape(upper.dataset), 'dataset shape');
  equal(lower.provenance.source_workload_names, upper.provenance.source_workload_names, 'source oracle workload ordering');
  equal(lower.read.map((item) => item.prototype), upper.read.map((item) => item.prototype), 'read prototype ordering');
  equal(lower.mutation.map((item) => item.prototype), upper.mutation.map((item) => item.prototype), 'mutation prototype ordering');
  equal(lower.maintenance.map((item) => item.prototype), upper.maintenance.map((item) => item.prototype), 'maintenance prototype ordering');
  equal(
    lower.read.map((item) => item.workloads.map((entry) => entry.name)),
    upper.read.map((item) => item.workloads.map((entry) => entry.name)),
    'read workload ordering',
  );
  equal(
    lower.mutation.map((item) => item.workloads.map((entry) => entry.name)),
    upper.mutation.map((item) => item.workloads.map((entry) => entry.name)),
    'mutation workload ordering',
  );
  equal(mutationEffectsOf(lower.mutation), mutationEffectsOf(upper.mutation), 'mutation effect contract');
  return {
    required_scales_present: true,
    same_packet_contract_version: true,
    same_repository: true,
    same_commit: true,
    same_postgres_image: true,
    same_repetitions: true,
    same_churn_cycles: true,
    same_database_settings: true,
    same_dataset_shape: true,
    same_source_oracle_shape: true,
    same_report_shape: true,
    same_mutation_effect_contract: true,
  };
};

const crossScale = (scales) => {
  const lower = scales.find((item) => item.scale === '100k');
  const upper = scales.find((item) => item.scale === '1m');
  if (!lower || !upper) return null;
  return {
    source_workloads: lower.source_workloads.map((entry) => ({
      name: entry.name,
      result_rows_ratio_1m_to_100k: ratio(
        upper.source_workloads.find((item) => item.name === entry.name).result_rows,
        entry.result_rows,
      ),
    })),
    prototypes: lower.read.map((read) => {
      const name = read.prototype;
      const read1m = candidate(upper, 'read', name);
      const mutation100k = candidate(lower, 'mutation', name);
      const mutation1m = candidate(upper, 'mutation', name);
      const maintenance100k = candidate(lower, 'maintenance', name);
      const maintenance1m = candidate(upper, 'maintenance', name);
      return {
        prototype: name,
        load_ms_ratio_1m_to_100k: ratio(read1m.load_ms, read.load_ms),
        schema_bytes_ratio_1m_to_100k: ratio(read1m.schema_bytes, read.schema_bytes),
        field_rows_ratio_1m_to_100k: maintenance100k.after_churn.field_rows === null
          ? null
          : ratio(maintenance1m.after_churn.field_rows, maintenance100k.after_churn.field_rows),
        vacuum_duration_ratio_1m_to_100k: ratio(
          maintenance1m.vacuum_duration_ms,
          maintenance100k.vacuum_duration_ms,
        ),
        read_workloads: read.workloads.map((entry) => ({
          name: entry.name,
          warm_execution_ratio_1m_to_100k: ratio(
            workload(read1m, entry.name).warm_median_execution_ms,
            entry.warm_median_execution_ms,
          ),
        })),
        mutation_workloads: mutation100k.workloads.map((entry) => ({
          name: entry.name,
          execution_ratio_1m_to_100k: ratio(
            workload(mutation1m, entry.name).median_execution_ms,
            entry.median_execution_ms,
          ),
          wal_bytes_ratio_1m_to_100k: ratio(
            workload(mutation1m, entry.name).median_maximum_node_wal_bytes,
            entry.median_maximum_node_wal_bytes,
          ),
        })),
      };
    }),
  };
};

const fixed = (value, digits = 2) => Number.isFinite(value) ? value.toFixed(digits) : 'n/a';
const integer = (value) => Number.isFinite(value) ? Math.round(value).toLocaleString('en-US') : 'n/a';
const bytes = (value) => {
  if (!Number.isFinite(value)) return 'n/a';
  const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
  let amount = value;
  let index = 0;
  while (Math.abs(amount) >= 1024 && index < units.length - 1) {
    amount /= 1024;
    index += 1;
  }
  return `${amount.toFixed(index === 0 ? 0 : 2)} ${units[index]}`;
};

const markdown = (report) => {
  const lines = [
    '# Index storage evidence comparison',
    '',
    `Generated: ${report.generated_at}`,
    '',
    '> Evidence summary only. The first repetition is a first-run signal and later repetitions form the warm median; this is not a guaranteed OS cold-cache test.',
    '',
    `Decision ready: **${report.decision_ready ? 'yes' : 'no'}**`,
    '',
    '## Decision contract',
    '',
    `- Required 100k/1m scales: **${report.decision_contract.required_scales_present ? 'yes' : 'no'}**`,
    `- Same packet contract version: **${report.decision_contract.same_packet_contract_version === true ? 'yes' : 'n/a'}**`,
    `- Same repository: **${report.decision_contract.same_repository === true ? 'yes' : 'n/a'}**`,
    `- Same commit: **${report.decision_contract.same_commit === true ? 'yes' : 'n/a'}**`,
    `- Same PostgreSQL image/settings: **${report.decision_contract.same_postgres_image === true && report.decision_contract.same_database_settings === true ? 'yes' : 'n/a'}**`,
    `- Same repetitions/churn contract: **${report.decision_contract.same_repetitions === true && report.decision_contract.same_churn_cycles === true ? 'yes' : 'n/a'}**`,
    `- Same non-scale dataset shape: **${report.decision_contract.same_dataset_shape === true ? 'yes' : 'n/a'}**`,
    `- Same source-oracle shape: **${report.decision_contract.same_source_oracle_shape === true ? 'yes' : 'n/a'}**`,
    `- Same candidate/workload shape: **${report.decision_contract.same_report_shape === true ? 'yes' : 'n/a'}**`,
    `- Same mutation effect contract: **${report.decision_contract.same_mutation_effect_contract === true ? 'yes' : 'n/a'}**`,
    '',
  ];
  for (const scale of report.scales) {
    lines.push(
      `## ${scale.scale} evidence`,
      '',
      `- Packet contract: \`v${scale.provenance.packet_contract_version}\``,
      `- Repository: \`${scale.provenance.repository ?? 'unknown'}\``,
      `- Commit: \`${scale.provenance.commit ?? 'unknown'}\``,
      `- Workflow run: \`${scale.provenance.run_id ?? 'unknown'}\``,
      `- PostgreSQL image: \`${scale.provenance.postgres_image}\``,
      `- Source load: ${fixed(scale.source_load_ms, 0)} ms`,
      '',
      '### Source oracle',
      '',
      '| Workload | Result rows | Digest |',
      '| --- | ---: | --- |',
    );
    for (const entry of scale.source_workloads) {
      lines.push(`| ${entry.name} | ${integer(entry.result_rows)} | \`${entry.result_digest}\` |`);
    }
    lines.push(
      '',
      '| Prototype | Load | Schema size | Fields after churn | Churn growth | Dead tuples after churn | VACUUM |',
      '| --- | ---: | ---: | ---: | ---: | ---: | ---: |',
    );
    for (const read of scale.read) {
      const maintenance = candidate(scale, 'maintenance', read.prototype);
      lines.push(`| ${read.prototype} | ${fixed(read.load_ms, 0)} ms | ${bytes(read.schema_bytes)} | ${integer(maintenance.after_churn.field_rows)} | ${bytes(maintenance.churn_growth_bytes)} (${fixed(maintenance.churn_growth_percent)}%) | ${integer(maintenance.after_churn.estimated_dead_tuples)} | ${fixed(maintenance.vacuum_duration_ms, 0)} ms |`);
    }
    lines.push(
      '',
      '### Read/query',
      '',
      '| Prototype | Workload | First run | Warm median | First read blocks | Warm read blocks | Plan shapes |',
      '| --- | --- | ---: | ---: | ---: | ---: | ---: |',
    );
    for (const item of scale.read) {
      for (const entry of item.workloads) {
        lines.push(`| ${item.prototype} | ${entry.name} | ${fixed(entry.first_execution_ms)} ms | ${fixed(entry.warm_median_execution_ms)} ms | ${integer(entry.first_shared_read_blocks)} | ${integer(entry.warm_median_shared_read_blocks)} | ${entry.plan_shape_variants} |`);
      }
    }
    lines.push(
      '',
      '### Mutation/WAL',
      '',
      '| Prototype | Workload | Entities | Fields | Links | Median execution | Median WAL bytes (max node) | Peak WAL bytes (max node) | Plan shapes |',
      '| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |',
    );
    for (const item of scale.mutation) {
      for (const entry of item.workloads) {
        lines.push(`| ${item.prototype} | ${entry.name} | ${integer(entry.affected_entities)} | ${integer(entry.affected_fields)} | ${integer(entry.affected_links)} | ${fixed(entry.median_execution_ms)} ms | ${integer(entry.median_maximum_node_wal_bytes)} | ${integer(entry.peak_maximum_node_wal_bytes)} | ${entry.plan_shape_variants} |`);
      }
    }
    lines.push('');
  }
  if (report.cross_scale_ratios) {
    lines.push(
      '## 1m / 100k ratios',
      '',
      '### Source oracle result rows',
      '',
      '| Workload | Result rows |',
      '| --- | ---: |',
    );
    for (const item of report.cross_scale_ratios.source_workloads) {
      lines.push(`| ${item.name} | ${fixed(item.result_rows_ratio_1m_to_100k)}x |`);
    }
    lines.push(
      '',
      '### Storage candidates',
      '',
      '| Prototype | Load | Schema | Field rows | VACUUM |',
      '| --- | ---: | ---: | ---: | ---: |',
    );
    for (const item of report.cross_scale_ratios.prototypes) {
      lines.push(`| ${item.prototype} | ${fixed(item.load_ms_ratio_1m_to_100k)}x | ${fixed(item.schema_bytes_ratio_1m_to_100k)}x | ${fixed(item.field_rows_ratio_1m_to_100k)}x | ${fixed(item.vacuum_duration_ratio_1m_to_100k)}x |`);
    }
    lines.push('');
  }
  lines.push(
    '## Manual ADR inputs still required',
    '',
    '- operational complexity and schema-evolution cost;',
    '- index-management and migration strategy;',
    '- acceptable latency, relation-size, WAL and maintenance trade-offs;',
    '- selected model and explicit rejection rationale for the alternatives.',
    '',
  );
  return `${lines.join('\n')}\n`;
};

const { inputs, output } = parseArgs();
const order = ['smoke', '100k', '1m'];
const scales = inputs.map(loadScale).sort((left, right) => order.indexOf(left.scale) - order.indexOf(right.scale));
if (new Set(scales.map((item) => item.scale)).size !== scales.length) die('duplicate scale input');
const decisionContract = requireDecisionProvenance(scales);
const report = {
  generated_at: new Date().toISOString(),
  methodology: {
    source_oracle: 'normalized idx_bench_source workload result digests',
    evidence_validation: 'fail closed on report shape, metrics, plans, effects, and cardinalities',
    first_run: 'first EXPLAIN ANALYZE repetition',
    warm_run: 'median after the first repetition; not a guaranteed OS cold-cache comparison',
    automatic_winner_selection: false,
  },
  decision_ready: decisionContract.required_scales_present,
  decision_contract: decisionContract,
  scales,
  cross_scale_ratios: crossScale(scales),
};
mkdirSync(output, { recursive: true });
writeFileSync(path.join(output, 'comparison.json'), `${JSON.stringify(report, null, 2)}\n`);
writeFileSync(path.join(output, 'comparison.md'), markdown(report));
console.log(`[compare-index-storage-evidence] wrote comparison.json and comparison.md; decision_ready=${report.decision_ready}`);
