import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const die = (message) => {
  console.error(`[compare-index-storage-evidence] ${message}`);
  process.exit(1);
};

const canonicalReadWorkloads = [
  'status_equality',
  'price_range_sort',
  'multi_value_tag',
  'two_hop_channel_filter',
  'keyset_page',
  'exact_count',
];
const comparableDatabaseFields = [
  'server_version_num',
  'shared_buffers',
  'effective_cache_size',
  'work_mem',
  'random_page_cost',
  'jit',
];
const requiredDatabaseFields = ['version', ...comparableDatabaseFields];

const parseArgs = () => {
  const inputs = [];
  let output = 'evidence/index-storage/comparison';
  const args = process.argv.slice(2);
  for (let i = 0; i < args.length; i += 1) {
    if (args[i] === '--input' && args[i + 1]) inputs.push(args[++i]);
    else if (args[i] === '--output' && args[i + 1]) output = args[++i];
    else if (args[i] === '--help' || args[i] === '-h') {
      console.log('Usage: node scripts/verify/compare-index-storage-evidence.mjs --input <dir> [--input <dir>] [--output <dir>]');
      process.exit(0);
    } else die(`unknown or incomplete argument: ${args[i]}`);
  }
  if (inputs.length === 0) die('at least one --input directory is required');
  return { inputs, output };
};

const json = (directory, filename) => {
  const file = path.join(directory, filename);
  if (!existsSync(file)) die(`missing evidence file: ${file}`);
  try {
    return JSON.parse(readFileSync(file, 'utf8'));
  } catch (error) {
    die(`invalid JSON in ${file}: ${error.message}`);
  }
};

const sameJson = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const numbers = (values) => values.filter(Number.isFinite);
const median = (values) => {
  const sorted = numbers(values).sort((a, b) => a - b);
  if (sorted.length === 0) return null;
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
};
const max = (values) => {
  const filtered = numbers(values);
  return filtered.length ? Math.max(...filtered) : null;
};
const sum = (values) => numbers(values).reduce((total, value) => total + value, 0);
const ratio = (a, b) => Number.isFinite(a) && Number.isFinite(b) && b !== 0 ? a / b : null;
const percent = (delta, base) => {
  const value = ratio(delta, base);
  return value === null ? null : value * 100;
};
const scaleName = (value) => ({
  smoke: 'smoke', Smoke: 'smoke', rows100k: '100k', Rows100k: '100k', '100k': '100k',
  rows1m: '1m', Rows1m: '1m', '1m': '1m',
}[value]);

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
const requirePositiveInteger = (value, label) => {
  if (!Number.isInteger(value) || value <= 0) die(`${label} must be a positive integer`);
};
const requireNonNegativeInteger = (value, label) => {
  if (!Number.isInteger(value) || value < 0) die(`${label} must be a non-negative integer`);
};
const requireDigest = (value, label) => {
  if (typeof value !== 'string' || !/^[0-9a-f]{32}$/.test(value)) die(`${label} must be an MD5 digest`);
};
const requireExactOrder = (actual, expected, label) => {
  requireArray(actual, label);
  if (new Set(actual).size !== actual.length) die(`${label} contains duplicate entries`);
  if (!sameJson(actual, expected)) {
    die(`${label} mismatch: expected ${expected.join(', ')}, got ${actual.join(', ')}`);
  }
};

const planShape = (plan) => {
  const root = (Array.isArray(plan) ? plan[0] : plan)?.Plan ?? plan;
  const walk = (node) => !node || typeof node !== 'object' ? null : ({
    node: node['Node Type'] ?? null,
    relation: node['Relation Name'] ?? null,
    index: node['Index Name'] ?? null,
    join: node['Join Type'] ?? null,
    strategy: node.Strategy ?? null,
    plans: Array.isArray(node.Plans) ? node.Plans.map(walk) : [],
  });
  return JSON.stringify(walk(root));
};

const explain = (repetitions) => {
  requireArray(repetitions, 'EXPLAIN repetitions');
  const warm = repetitions.length > 1 ? repetitions.slice(1) : repetitions;
  return {
    repetitions: repetitions.length,
    first_execution_ms: repetitions[0]?.execution_time_ms ?? null,
    warm_median_execution_ms: median(warm.map((item) => item.execution_time_ms)),
    median_execution_ms: median(repetitions.map((item) => item.execution_time_ms)),
    median_planning_ms: median(repetitions.map((item) => item.planning_time_ms)),
    first_shared_read_blocks: repetitions[0]?.shared_read_blocks ?? null,
    warm_median_shared_read_blocks: median(warm.map((item) => item.shared_read_blocks)),
    warm_median_shared_hit_blocks: median(warm.map((item) => item.shared_hit_blocks)),
    median_temp_read_blocks: median(repetitions.map((item) => item.temporary_read_blocks)),
    median_temp_written_blocks: median(repetitions.map((item) => item.temporary_written_blocks)),
    plan_shape_variants: new Set(repetitions.map((item) => planShape(item.plan))).size,
  };
};

const snapshot = (value) => ({
  schema_bytes: value.schema_bytes,
  entity_rows: value.entity_rows,
  field_rows: value.field_rows,
  link_rows: value.link_rows,
  estimated_live_tuples: sum(value.table_stats.map((item) => item.estimated_live_tuples)),
  estimated_dead_tuples: sum(value.table_stats.map((item) => item.estimated_dead_tuples)),
  tuples_inserted: sum(value.table_stats.map((item) => item.tuples_inserted)),
  tuples_updated: sum(value.table_stats.map((item) => item.tuples_updated)),
  tuples_deleted: sum(value.table_stats.map((item) => item.tuples_deleted)),
  hot_updates: sum(value.table_stats.map((item) => item.hot_updates)),
});

const validateDatabase = (database, scale) => {
  requireObject(database, `${scale} database`);
  for (const field of requiredDatabaseFields) {
    requireNonEmptyString(database[field], `${scale} database.${field}`);
  }
  const serverVersion = Number.parseInt(database.server_version_num, 10);
  if (!Number.isInteger(serverVersion) || Math.floor(serverVersion / 10_000) !== 16) {
    die(`${scale} database.server_version_num must describe PostgreSQL 16`);
  }
  if (database.jit !== 'off') die(`${scale} database.jit must be off`);
};

const validateDataset = (dataset, provenance, scale) => {
  requireObject(dataset, `${scale} dataset`);
  for (const field of [
    'tenants',
    'products_per_tenant',
    'variants_per_product',
    'channels_per_tenant',
    'sales_channels_per_variant',
  ]) {
    requirePositiveInteger(dataset[field], `${scale} dataset.${field}`);
  }
  requireArray(dataset.locales, `${scale} dataset.locales`);
  if (dataset.locales.length === 0 || new Set(dataset.locales).size !== dataset.locales.length) {
    die(`${scale} dataset.locales must contain unique locale values`);
  }
  dataset.locales.forEach((locale, index) => requireNonEmptyString(locale, `${scale} dataset.locales[${index}]`));

  const productRows = dataset.tenants * dataset.products_per_tenant * dataset.locales.length;
  const variantRows = productRows * dataset.variants_per_product;
  const channelRows = dataset.tenants * dataset.channels_per_tenant;
  const entityRows = productRows + variantRows + channelRows;
  const eavFieldRows = productRows * 8 + variantRows * 3 + channelRows * 2;
  const linkRows = variantRows + variantRows * dataset.sales_channels_per_variant;
  const derived = {
    expected_product_rows: productRows,
    expected_entity_rows: entityRows,
    expected_eav_field_rows: eavFieldRows,
    expected_link_rows: linkRows,
  };
  for (const [field, value] of Object.entries(derived)) {
    if (provenance[field] !== value) {
      die(`${scale} dataset ${field} mismatch: provenance=${provenance[field]} derived=${value}`);
    }
  }
  return dataset;
};

const validateExecutionContract = (read, mutation, maintenance, provenance, scale) => {
  requirePositiveInteger(provenance.repetitions, `${scale} provenance.repetitions`);
  requirePositiveInteger(provenance.churn_cycles, `${scale} provenance.churn_cycles`);
  if (mutation.repetitions !== provenance.repetitions) {
    die(`${scale} repetitions mismatch: provenance=${provenance.repetitions} mutation=${mutation.repetitions}`);
  }
  if (maintenance.cycles !== provenance.churn_cycles) {
    die(`${scale} churn_cycles mismatch: provenance=${provenance.churn_cycles} maintenance=${maintenance.cycles}`);
  }
  for (const prototype of requireArray(read.prototypes, `${scale} read prototypes`)) {
    for (const workload of requireArray(prototype.workloads, `${scale}/${prototype.prototype} read workloads`)) {
      if (!Array.isArray(workload.repetitions)
          || workload.repetitions.length !== provenance.repetitions) {
        die(`${scale}/${prototype.prototype}/${workload.name} read repetitions mismatch`);
      }
    }
  }
  for (const prototype of requireArray(mutation.prototypes, `${scale} mutation prototypes`)) {
    for (const workload of requireArray(prototype.workloads, `${scale}/${prototype.prototype} mutation workloads`)) {
      if (!Array.isArray(workload.repetitions)
          || workload.repetitions.length !== provenance.repetitions) {
        die(`${scale}/${prototype.prototype}/${workload.name} mutation repetitions mismatch`);
      }
    }
  }
};

const validateSourceOracle = (read, provenance, scale) => {
  const sourceWorkloads = requireArray(read.source_workloads, `${scale} source_workloads`);
  const names = sourceWorkloads.map((item) => item?.name);
  requireExactOrder(names, canonicalReadWorkloads, `${scale} source workload order`);
  requireExactOrder(
    provenance.source_workload_names,
    canonicalReadWorkloads,
    `${scale} provenance source workload order`,
  );

  const oracle = new Map();
  for (const item of sourceWorkloads) {
    requireObject(item, `${scale} source/${item?.name ?? 'unknown'}`);
    requireNonEmptyString(item.sql, `${scale} source/${item.name}.sql`);
    if (!item.sql.includes('idx_bench_source.')) {
      die(`${scale} source/${item.name}.sql must read from idx_bench_source`);
    }
    requireNonNegativeInteger(item.result_rows, `${scale} source/${item.name}.result_rows`);
    requireDigest(item.result_digest, `${scale} source/${item.name}.result_digest`);
    oracle.set(item.name, item);
  }

  for (const prototype of read.prototypes) {
    requireExactOrder(
      prototype.workloads?.map((item) => item?.name),
      names,
      `${scale}/${prototype.prototype} read workload order`,
    );
    if (prototype.entity_rows !== provenance.expected_entity_rows
        || prototype.link_rows !== provenance.expected_link_rows) {
      die(`${scale}/${prototype.prototype} read cardinality mismatch`);
    }
    for (const workload of prototype.workloads) {
      const expected = oracle.get(workload.name);
      requireNonNegativeInteger(workload.result_rows, `${scale}/${prototype.prototype}/${workload.name}.result_rows`);
      requireDigest(workload.result_digest, `${scale}/${prototype.prototype}/${workload.name}.result_digest`);
      if (!expected
          || workload.result_rows !== expected.result_rows
          || workload.result_digest !== expected.result_digest) {
        die(`${scale}/${prototype.prototype}/${workload.name} differs from source oracle`);
      }
    }
  }

  return sourceWorkloads.map((item) => ({
    name: item.name,
    sql: item.sql,
    result_rows: item.result_rows,
    result_digest: item.result_digest,
  }));
};

const loadScale = (directory) => {
  const read = requireObject(json(directory, 'read-report.json'), `${directory}/read-report.json`);
  const mutation = requireObject(json(directory, 'mutation-report.json'), `${directory}/mutation-report.json`);
  const maintenance = requireObject(json(directory, 'maintenance-report.json'), `${directory}/maintenance-report.json`);
  const provenance = requireObject(json(directory, 'provenance.json'), `${directory}/provenance.json`);
  const names = [read.dataset?.scale, mutation.dataset_scale, maintenance.dataset_scale, provenance.scale]
    .map(scaleName);
  if (names.some((name) => !name) || new Set(names).size !== 1) {
    die(`scale mismatch in ${directory}: ${names.join(', ')}`);
  }
  const scale = names[0];
  if (provenance.packet_contract_version !== 2) {
    die(`${scale} evidence must use packet contract version 2`);
  }
  for (const field of [
    'expected_product_rows',
    'expected_entity_rows',
    'expected_eav_field_rows',
    'expected_link_rows',
  ]) {
    requirePositiveInteger(provenance[field], `${scale} provenance.${field}`);
  }

  validateDatabase(read.database, scale);
  const dataset = validateDataset(read.dataset, provenance, scale);
  validateExecutionContract(read, mutation, maintenance, provenance, scale);
  if (read.source_entity_rows !== provenance.expected_entity_rows
      || read.source_link_rows !== provenance.expected_link_rows) {
    die(`${scale} source cardinality does not match provenance`);
  }

  const sourceWorkloads = validateSourceOracle(read, provenance, scale);
  const prototypes = read.prototypes.map((item) => item.prototype);
  for (const report of [mutation, maintenance]) {
    if (!sameJson(report.prototypes.map((item) => item.prototype), prototypes)) {
      die(`prototype ordering mismatch in ${directory}`);
    }
  }
  for (const item of maintenance.prototypes) {
    const expectedFieldRows = item.prototype === 'typed_eav'
      ? provenance.expected_eav_field_rows
      : null;
    for (const phase of ['baseline', 'after_churn', 'after_vacuum']) {
      const state = item[phase];
      if (state.entity_rows !== provenance.expected_entity_rows
          || state.field_rows !== expectedFieldRows
          || state.link_rows !== provenance.expected_link_rows) {
        die(`${scale}/${item.prototype}/${phase} maintenance cardinality mismatch`);
      }
    }
  }

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
      postgres_image: provenance.postgres_image ?? null,
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
    source_workloads: sourceWorkloads,
    read: read.prototypes.map((item) => ({
      prototype: item.prototype,
      schema: item.schema,
      load_ms: item.load_ms,
      schema_bytes: item.schema_bytes,
      entity_rows: item.entity_rows,
      link_rows: item.link_rows,
      workloads: item.workloads.map((workload) => ({
        name: workload.name,
        result_rows: workload.result_rows,
        result_digest: workload.result_digest,
        ...explain(workload.repetitions),
      })),
    })),
    mutation: mutation.prototypes.map((item) => ({
      prototype: item.prototype,
      schema: item.schema,
      workloads: item.workloads.map((workload) => ({
        name: workload.name,
        affected_entities: workload.affected_entities,
        affected_fields: workload.affected_fields,
        affected_links: workload.affected_links,
        ...explain(workload.repetitions),
        median_maximum_node_wal_records: median(workload.repetitions.map((r) => r.maximum_node_wal_records)),
        median_maximum_node_wal_fpi: median(workload.repetitions.map((r) => r.maximum_node_wal_fpi)),
        median_maximum_node_wal_bytes: median(workload.repetitions.map((r) => r.maximum_node_wal_bytes)),
        peak_maximum_node_wal_bytes: max(workload.repetitions.map((r) => r.maximum_node_wal_bytes)),
      })),
    })),
    maintenance: maintenance.prototypes.map((item) => {
      const baseline = snapshot(item.baseline);
      const afterChurn = snapshot(item.after_churn);
      const afterVacuum = snapshot(item.after_vacuum);
      const sizeDelta = afterVacuum.schema_bytes - afterChurn.schema_bytes;
      return {
        prototype: item.prototype,
        schema: item.schema,
        baseline,
        after_churn: afterChurn,
        after_vacuum: afterVacuum,
        churn_growth_bytes: afterChurn.schema_bytes - baseline.schema_bytes,
        churn_growth_percent: percent(afterChurn.schema_bytes - baseline.schema_bytes, baseline.schema_bytes),
        vacuum_size_delta_bytes: sizeDelta,
        vacuum_size_delta_percent: percent(sizeDelta, afterChurn.schema_bytes),
        vacuum_duration_ms: item.vacuum_duration_ms,
      };
    }),
  };
};

const candidate = (scale, section, name) => scale[section].find((item) => item.prototype === name);
const workload = (item, name) => item.workloads.find((entry) => entry.name === name);
const namesOf = (items) => items.map((item) => item.prototype);
const workloadNamesOf = (items) => Object.fromEntries(
  items.map((item) => [item.prototype, item.workloads.map((entry) => entry.name)]),
);
const mutationEffectsOf = (items) => Object.fromEntries(items.map((item) => [
  item.prototype,
  item.workloads.map((entry) => ({
    name: entry.name,
    affected_entities: entry.affected_entities,
    affected_fields: entry.affected_fields,
    affected_links: entry.affected_links,
  })),
]));
const datasetShape = (dataset) => ({
  locales: dataset.locales,
  variants_per_product: dataset.variants_per_product,
  channels_per_tenant: dataset.channels_per_tenant,
  sales_channels_per_variant: dataset.sales_channels_per_variant,
});

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

  const requiredText = ['repository', 'commit', 'postgres_image'];
  for (const scale of [lower, upper]) {
    for (const field of requiredText) {
      if (typeof scale.provenance[field] !== 'string' || scale.provenance[field].length === 0) {
        die(`${scale.scale} provenance is missing ${field}`);
      }
    }
  }

  const equalField = (field, label = field) => {
    if (lower.provenance[field] !== upper.provenance[field]) {
      die(`cross-scale provenance ${label} mismatch: 100k=${lower.provenance[field]} 1m=${upper.provenance[field]}`);
    }
  };
  equalField('packet_contract_version', 'packet contract version');
  equalField('repository');
  equalField('commit');
  equalField('postgres_image', 'PostgreSQL image');
  equalField('repetitions');
  equalField('churn_cycles');

  for (const field of comparableDatabaseFields) {
    if (lower.database[field] !== upper.database[field]) {
      die(`cross-scale database setting ${field} mismatch: 100k=${lower.database[field]} 1m=${upper.database[field]}`);
    }
  }
  if (!sameJson(datasetShape(lower.dataset), datasetShape(upper.dataset))) {
    die('cross-scale dataset shape mismatch');
  }
  if (!sameJson(
    lower.source_workloads.map((item) => item.name),
    upper.source_workloads.map((item) => item.name),
  )) {
    die('cross-scale source oracle workload ordering mismatch');
  }
  for (const section of ['read', 'mutation', 'maintenance']) {
    if (!sameJson(namesOf(lower[section]), namesOf(upper[section]))) {
      die(`cross-scale ${section} prototype ordering mismatch`);
    }
  }
  if (!sameJson(workloadNamesOf(lower.read), workloadNamesOf(upper.read))) {
    die('cross-scale read workload ordering mismatch');
  }
  if (!sameJson(workloadNamesOf(lower.mutation), workloadNamesOf(upper.mutation))) {
    die('cross-scale mutation workload ordering mismatch');
  }
  if (!sameJson(mutationEffectsOf(lower.mutation), mutationEffectsOf(upper.mutation))) {
    die('cross-scale mutation effect contract mismatch');
  }

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
        upper.source_workloads.find((item) => item.name === entry.name)?.result_rows,
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
        field_rows_ratio_1m_to_100k: ratio(
          maintenance1m.after_churn.field_rows,
          maintenance100k.after_churn.field_rows,
        ),
        vacuum_duration_ratio_1m_to_100k: ratio(
          maintenance1m.vacuum_duration_ms,
          maintenance100k.vacuum_duration_ms,
        ),
        read_workloads: read.workloads.map((entry) => ({
          name: entry.name,
          warm_execution_ratio_1m_to_100k: ratio(
            workload(read1m, entry.name)?.warm_median_execution_ms,
            entry.warm_median_execution_ms,
          ),
        })),
        mutation_workloads: mutation100k.workloads.map((entry) => ({
          name: entry.name,
          execution_ratio_1m_to_100k: ratio(
            workload(mutation1m, entry.name)?.median_execution_ms,
            entry.median_execution_ms,
          ),
          wal_bytes_ratio_1m_to_100k: ratio(
            workload(mutation1m, entry.name)?.median_maximum_node_wal_bytes,
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
  while (Math.abs(amount) >= 1024 && index < units.length - 1) { amount /= 1024; index += 1; }
  return `${amount.toFixed(index === 0 ? 0 : 2)} ${units[index]}`;
};

const markdown = (report) => {
  const lines = [
    '# Index storage evidence comparison', '', `Generated: ${report.generated_at}`, '',
    '> Evidence summary only. The first repetition is a first-run signal and later repetitions form the warm median; this is not a guaranteed OS cold-cache test.',
    '', `Decision ready: **${report.decision_ready ? 'yes' : 'no'}**`, '',
    '## Decision contract', '',
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
    lines.push(`## ${scale.scale} evidence`, '',
      `- Packet contract: \`v${scale.provenance.packet_contract_version}\``,
      `- Repository: \`${scale.provenance.repository ?? 'unknown'}\``,
      `- Commit: \`${scale.provenance.commit ?? 'unknown'}\``,
      `- Workflow run: \`${scale.provenance.run_id ?? 'unknown'}\``,
      `- PostgreSQL image: \`${scale.provenance.postgres_image ?? 'unknown'}\``,
      `- Source load: ${fixed(scale.source_load_ms, 0)} ms`, '',
      '### Source oracle', '',
      '| Workload | Result rows | Digest |',
      '| --- | ---: | --- |');
    for (const entry of scale.source_workloads) {
      lines.push(`| ${entry.name} | ${integer(entry.result_rows)} | \`${entry.result_digest}\` |`);
    }
    lines.push('',
      '| Prototype | Load | Schema size | Fields after churn | Churn growth | Dead tuples after churn | VACUUM |',
      '| --- | ---: | ---: | ---: | ---: | ---: | ---: |');
    for (const read of scale.read) {
      const maintenance = candidate(scale, 'maintenance', read.prototype);
      lines.push(`| ${read.prototype} | ${fixed(read.load_ms, 0)} ms | ${bytes(read.schema_bytes)} | ${integer(maintenance.after_churn.field_rows)} | ${bytes(maintenance.churn_growth_bytes)} (${fixed(maintenance.churn_growth_percent)}%) | ${integer(maintenance.after_churn.estimated_dead_tuples)} | ${fixed(maintenance.vacuum_duration_ms, 0)} ms |`);
    }
    lines.push('', '### Read/query', '',
      '| Prototype | Workload | First run | Warm median | First read blocks | Warm read blocks | Plan shapes |',
      '| --- | --- | ---: | ---: | ---: | ---: | ---: |');
    for (const item of scale.read) for (const entry of item.workloads) {
      lines.push(`| ${item.prototype} | ${entry.name} | ${fixed(entry.first_execution_ms)} ms | ${fixed(entry.warm_median_execution_ms)} ms | ${integer(entry.first_shared_read_blocks)} | ${integer(entry.warm_median_shared_read_blocks)} | ${entry.plan_shape_variants} |`);
    }
    lines.push('', '### Mutation/WAL', '',
      '| Prototype | Workload | Entities | Fields | Links | Median execution | Median WAL bytes (max node) | Peak WAL bytes (max node) | Plan shapes |',
      '| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |');
    for (const item of scale.mutation) for (const entry of item.workloads) {
      lines.push(`| ${item.prototype} | ${entry.name} | ${integer(entry.affected_entities)} | ${integer(entry.affected_fields)} | ${integer(entry.affected_links)} | ${fixed(entry.median_execution_ms)} ms | ${integer(entry.median_maximum_node_wal_bytes)} | ${integer(entry.peak_maximum_node_wal_bytes)} | ${entry.plan_shape_variants} |`);
    }
    lines.push('');
  }
  if (report.cross_scale_ratios) {
    lines.push('## 1m / 100k ratios', '', '### Source oracle result rows', '',
      '| Workload | Result rows |', '| --- | ---: |');
    for (const item of report.cross_scale_ratios.source_workloads) {
      lines.push(`| ${item.name} | ${fixed(item.result_rows_ratio_1m_to_100k)}x |`);
    }
    lines.push('', '### Storage candidates', '',
      '| Prototype | Load | Schema | Field rows | VACUUM |',
      '| --- | ---: | ---: | ---: | ---: |');
    for (const item of report.cross_scale_ratios.prototypes) {
      lines.push(`| ${item.prototype} | ${fixed(item.load_ms_ratio_1m_to_100k)}x | ${fixed(item.schema_bytes_ratio_1m_to_100k)}x | ${fixed(item.field_rows_ratio_1m_to_100k)}x | ${fixed(item.vacuum_duration_ratio_1m_to_100k)}x |`);
    }
    lines.push('');
  }
  lines.push('## Manual ADR inputs still required', '',
    '- operational complexity and schema-evolution cost;',
    '- index-management and migration strategy;',
    '- acceptable latency, relation-size, WAL and maintenance trade-offs;',
    '- selected model and explicit rejection rationale for the alternatives.', '');
  return `${lines.join('\n')}\n`;
};

const { inputs, output } = parseArgs();
const order = ['smoke', '100k', '1m'];
const scales = inputs.map(loadScale).sort((a, b) => order.indexOf(a.scale) - order.indexOf(b.scale));
if (new Set(scales.map((item) => item.scale)).size !== scales.length) die('duplicate scale input');
const decisionContract = requireDecisionProvenance(scales);
const report = {
  generated_at: new Date().toISOString(),
  methodology: {
    source_oracle: 'normalized idx_bench_source workload result digests',
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