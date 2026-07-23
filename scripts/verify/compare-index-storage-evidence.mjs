import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const fail = (message) => {
  console.error(`[compare-index-storage-evidence] ${message}`);
  process.exit(1);
};

const parseArgs = (argv) => {
  const inputs = [];
  let output = 'evidence/index-storage/comparison';
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === '--input') {
      const value = argv[index + 1];
      if (!value) fail('--input requires a directory');
      inputs.push(value);
      index += 1;
    } else if (argument === '--output') {
      const value = argv[index + 1];
      if (!value) fail('--output requires a directory');
      output = value;
      index += 1;
    } else if (argument === '--help' || argument === '-h') {
      console.log(
        'Usage: node scripts/verify/compare-index-storage-evidence.mjs '
          + '--input <scale-dir> [--input <scale-dir>] [--output <dir>]',
      );
      process.exit(0);
    } else {
      fail(`unknown argument: ${argument}`);
    }
  }
  if (inputs.length === 0) fail('at least one --input directory is required');
  return { inputs, output };
};

const readJson = (directory, filename) => {
  const file = path.join(directory, filename);
  if (!existsSync(file)) fail(`missing evidence file: ${file}`);
  try {
    return JSON.parse(readFileSync(file, 'utf8'));
  } catch (error) {
    fail(`invalid JSON in ${file}: ${error.message}`);
  }
};

const compact = (values) => values.filter((value) => Number.isFinite(value));
const median = (values) => {
  const sorted = compact(values).sort((left, right) => left - right);
  if (sorted.length === 0) return null;
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0
    ? (sorted[middle - 1] + sorted[middle]) / 2
    : sorted[middle];
};
const maximum = (values) => {
  const numeric = compact(values);
  return numeric.length === 0 ? null : Math.max(...numeric);
};
const sum = (values) => compact(values).reduce((total, value) => total + value, 0);
const ratio = (numerator, denominator) => (
  Number.isFinite(numerator) && Number.isFinite(denominator) && denominator !== 0
    ? numerator / denominator
    : null
);
const deltaPercent = (after, before) => {
  const value = ratio(after - before, before);
  return value === null ? null : value * 100;
};

const normalizeScale = (value) => ({
  smoke: 'smoke',
  Smoke: 'smoke',
  rows100k: '100k',
  Rows100k: '100k',
  '100k': '100k',
  rows1m: '1m',
  Rows1m: '1m',
  '1m': '1m',
}[value]);

const planSignature = (value) => {
  const root = Array.isArray(value) ? value[0] : value;
  const walk = (node) => {
    if (!node || typeof node !== 'object') return null;
    return {
      node_type: node['Node Type'] ?? null,
      relation: node['Relation Name'] ?? null,
      alias: node.Alias ?? null,
      index: node['Index Name'] ?? null,
      join_type: node['Join Type'] ?? null,
      strategy: node.Strategy ?? null,
      parent_relationship: node['Parent Relationship'] ?? null,
      plans: Array.isArray(node.Plans) ? node.Plans.map(walk) : [],
    };
  };
  return JSON.stringify(walk(root?.Plan ?? root));
};

const summarizeExplainRepetitions = (repetitions) => {
  const warm = repetitions.length > 1 ? repetitions.slice(1) : repetitions;
  return {
    repetitions: repetitions.length,
    first_execution_ms: repetitions[0]?.execution_time_ms ?? null,
    warm_median_execution_ms: median(warm.map((item) => item.execution_time_ms)),
    median_execution_ms: median(repetitions.map((item) => item.execution_time_ms)),
    median_planning_ms: median(repetitions.map((item) => item.planning_time_ms)),
    first_shared_hit_blocks: repetitions[0]?.shared_hit_blocks ?? null,
    first_shared_read_blocks: repetitions[0]?.shared_read_blocks ?? null,
    warm_median_shared_hit_blocks: median(warm.map((item) => item.shared_hit_blocks)),
    warm_median_shared_read_blocks: median(warm.map((item) => item.shared_read_blocks)),
    median_temp_read_blocks: median(repetitions.map((item) => item.temporary_read_blocks)),
    median_temp_written_blocks: median(
      repetitions.map((item) => item.temporary_written_blocks),
    ),
    plan_shape_variants: new Set(
      repetitions.map((item) => planSignature(item.plan)),
    ).size,
  };
};

const summarizeReadPrototype = (prototype) => ({
  prototype: prototype.prototype,
  schema: prototype.schema,
  load_ms: prototype.load_ms,
  schema_bytes: prototype.schema_bytes,
  entity_rows: prototype.entity_rows,
  link_rows: prototype.link_rows,
  workloads: prototype.workloads.map((workload) => ({
    name: workload.name,
    result_rows: workload.result_rows,
    result_digest: workload.result_digest,
    ...summarizeExplainRepetitions(workload.repetitions),
  })),
});

const summarizeMutationPrototype = (prototype) => ({
  prototype: prototype.prototype,
  schema: prototype.schema,
  workloads: prototype.workloads.map((workload) => {
    const summary = summarizeExplainRepetitions(workload.repetitions);
    return {
      name: workload.name,
      affected_entities: workload.affected_entities,
      affected_links: workload.affected_links,
      ...summary,
      median_maximum_node_wal_records: median(
        workload.repetitions.map((item) => item.maximum_node_wal_records),
      ),
      median_maximum_node_wal_fpi: median(
        workload.repetitions.map((item) => item.maximum_node_wal_fpi),
      ),
      median_maximum_node_wal_bytes: median(
        workload.repetitions.map((item) => item.maximum_node_wal_bytes),
      ),
      peak_maximum_node_wal_bytes: maximum(
        workload.repetitions.map((item) => item.maximum_node_wal_bytes),
      ),
    };
  }),
});

const summarizeSnapshot = (snapshot) => ({
  schema_bytes: snapshot.schema_bytes,
  entity_rows: snapshot.entity_rows,
  link_rows: snapshot.link_rows,
  estimated_live_tuples: sum(
    snapshot.table_stats.map((item) => item.estimated_live_tuples),
  ),
  estimated_dead_tuples: sum(
    snapshot.table_stats.map((item) => item.estimated_dead_tuples),
  ),
  tuples_inserted: sum(snapshot.table_stats.map((item) => item.tuples_inserted)),
  tuples_updated: sum(snapshot.table_stats.map((item) => item.tuples_updated)),
  tuples_deleted: sum(snapshot.table_stats.map((item) => item.tuples_deleted)),
  hot_updates: sum(snapshot.table_stats.map((item) => item.hot_updates)),
});

const summarizeMaintenancePrototype = (prototype) => {
  const baseline = summarizeSnapshot(prototype.baseline);
  const afterChurn = summarizeSnapshot(prototype.after_churn);
  const afterVacuum = summarizeSnapshot(prototype.after_vacuum);
  return {
    prototype: prototype.prototype,
    schema: prototype.schema,
    baseline,
    after_churn: afterChurn,
    after_vacuum: afterVacuum,
    churn_growth_bytes: afterChurn.schema_bytes - baseline.schema_bytes,
    churn_growth_percent: deltaPercent(afterChurn.schema_bytes, baseline.schema_bytes),
    vacuum_reclaimed_bytes: afterChurn.schema_bytes - afterVacuum.schema_bytes,
    vacuum_reclaimed_percent: deltaPercent(afterVacuum.schema_bytes, afterChurn.schema_bytes),
    vacuum_duration_ms: prototype.vacuum_duration_ms,
  };
};

const loadScale = (directory) => {
  const read = readJson(directory, 'read-report.json');
  const mutation = readJson(directory, 'mutation-report.json');
  const maintenance = readJson(directory, 'maintenance-report.json');
  const provenance = readJson(directory, 'provenance.json');
  const scales = [
    normalizeScale(read.dataset?.scale),
    normalizeScale(mutation.dataset_scale),
    normalizeScale(maintenance.dataset_scale),
    normalizeScale(provenance.scale),
  ];
  if (scales.some((scale) => !scale) || new Set(scales).size !== 1) {
    fail(`scale mismatch in ${directory}: ${scales.join(', ')}`);
  }
  const scale = scales[0];
  const prototypeNames = read.prototypes.map((prototype) => prototype.prototype);
  for (const report of [mutation, maintenance]) {
    const names = report.prototypes.map((prototype) => prototype.prototype);
    if (JSON.stringify(names) !== JSON.stringify(prototypeNames)) {
      fail(`prototype ordering mismatch in ${directory}`);
    }
  }
  return {
    scale,
    directory,
    provenance: {
      commit: provenance.commit ?? null,
      run_id: provenance.run_id ?? null,
      run_attempt: provenance.run_attempt ?? null,
      postgres_image: provenance.postgres_image ?? null,
      runner_os: provenance.runner_os ?? null,
      runner_arch: provenance.runner_arch ?? null,
    },
    database: read.database,
    dataset: read.dataset,
    source_load_ms: read.source_load_ms,
    source_entity_rows: read.source_entity_rows,
    source_link_rows: read.source_link_rows,
    read: read.prototypes.map(summarizeReadPrototype),
    mutation: mutation.prototypes.map(summarizeMutationPrototype),
    maintenance: maintenance.prototypes.map(summarizeMaintenancePrototype),
  };
};

const findPrototype = (scale, section, prototype) => (
  scale[section].find((candidate) => candidate.prototype === prototype)
);
const findWorkload = (prototype, workload) => (
  prototype.workloads.find((candidate) => candidate.name === workload)
);

const buildCrossScaleRatios = (scales) => {
  const byScale = new Map(scales.map((scale) => [scale.scale, scale]));
  const lower = byScale.get('100k');
  const upper = byScale.get('1m');
  if (!lower || !upper) return null;
  return lower.read.map((prototype) => {
    const prototypeName = prototype.prototype;
    const lowerRead = findPrototype(lower, 'read', prototypeName);
    const upperRead = findPrototype(upper, 'read', prototypeName);
    const lowerMutation = findPrototype(lower, 'mutation', prototypeName);
    const upperMutation = findPrototype(upper, 'mutation', prototypeName);
    const lowerMaintenance = findPrototype(lower, 'maintenance', prototypeName);
    const upperMaintenance = findPrototype(upper, 'maintenance', prototypeName);
    return {
      prototype: prototypeName,
      load_ms_ratio_1m_to_100k: ratio(upperRead.load_ms, lowerRead.load_ms),
      schema_bytes_ratio_1m_to_100k: ratio(
        upperRead.schema_bytes,
        lowerRead.schema_bytes,
      ),
      vacuum_duration_ratio_1m_to_100k: ratio(
        upperMaintenance.vacuum_duration_ms,
        lowerMaintenance.vacuum_duration_ms,
      ),
      read_workloads: lowerRead.workloads.map((workload) => ({
        name: workload.name,
        warm_execution_ratio_1m_to_100k: ratio(
          findWorkload(upperRead, workload.name)?.warm_median_execution_ms,
          workload.warm_median_execution_ms,
        ),
      })),
      mutation_workloads: lowerMutation.workloads.map((workload) => ({
        name: workload.name,
        execution_ratio_1m_to_100k: ratio(
          findWorkload(upperMutation, workload.name)?.median_execution_ms,
          workload.median_execution_ms,
        ),
        wal_bytes_ratio_1m_to_100k: ratio(
          findWorkload(upperMutation, workload.name)?.median_maximum_node_wal_bytes,
          workload.median_maximum_node_wal_bytes,
        ),
      })),
    };
  });
};

const formatNumber = (value, digits = 2) => (
  Number.isFinite(value) ? value.toFixed(digits) : 'n/a'
);
const formatInteger = (value) => (
  Number.isFinite(value) ? Math.round(value).toLocaleString('en-US') : 'n/a'
);
const formatBytes = (value) => {
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

const renderMarkdown = (comparison) => {
  const lines = [
    '# Index storage evidence comparison',
    '',
    `Generated: ${comparison.generated_at}`,
    '',
    '> This report summarizes evidence; it does not select a storage model. '
      + 'The first repetition is reported separately from the median of later '
      + 'repetitions. That is a first-run/warm-run comparison, not a guaranteed '
      + 'operating-system cold-cache measurement.',
    '',
    `Decision ready: **${comparison.decision_ready ? 'yes' : 'no'}**`,
    '',
  ];
  for (const scale of comparison.scales) {
    lines.push(`## ${scale.scale} evidence`, '');
    lines.push(`- Commit: \`${scale.provenance.commit ?? 'unknown'}\``);
    lines.push(`- Workflow run: \`${scale.provenance.run_id ?? 'unknown'}\``);
    lines.push(
      `- PostgreSQL: \`${scale.provenance.postgres_image ?? scale.database.version}\``,
    );
    lines.push(`- Source load: ${formatNumber(scale.source_load_ms, 0)} ms`);
    lines.push('');
    lines.push(
      '| Prototype | Load | Schema size | Churn growth | Dead tuples after churn | VACUUM |',
    );
    lines.push('| --- | ---: | ---: | ---: | ---: | ---: |');
    for (const readPrototype of scale.read) {
      const maintenance = findPrototype(
        scale,
        'maintenance',
        readPrototype.prototype,
      );
      lines.push(
        `| ${readPrototype.prototype} | ${formatNumber(readPrototype.load_ms, 0)} ms `
          + `| ${formatBytes(readPrototype.schema_bytes)} `
          + `| ${formatBytes(maintenance.churn_growth_bytes)} `
          + `(${formatNumber(maintenance.churn_growth_percent)}%) `
          + `| ${formatInteger(maintenance.after_churn.estimated_dead_tuples)} `
          + `| ${formatNumber(maintenance.vacuum_duration_ms, 0)} ms |`,
      );
    }
    lines.push('', '### Read/query', '');
    lines.push(
      '| Prototype | Workload | First run | Warm median | First read blocks | Warm read blocks | Plan shapes |',
    );
    lines.push('| --- | --- | ---: | ---: | ---: | ---: | ---: |');
    for (const prototype of scale.read) {
      for (const workload of prototype.workloads) {
        lines.push(
          `| ${prototype.prototype} | ${workload.name} `
            + `| ${formatNumber(workload.first_execution_ms)} ms `
            + `| ${formatNumber(workload.warm_median_execution_ms)} ms `
            + `| ${formatInteger(workload.first_shared_read_blocks)} `
            + `| ${formatInteger(workload.warm_median_shared_read_blocks)} `
            + `| ${workload.plan_shape_variants} |`,
        );
      }
    }
    lines.push('', '### Mutation/WAL', '');
    lines.push(
      '| Prototype | Workload | Median execution | Median WAL bytes (max node) | Peak WAL bytes (max node) | Plan shapes |',
    );
    lines.push('| --- | --- | ---: | ---: | ---: | ---: |');
    for (const prototype of scale.mutation) {
      for (const workload of prototype.workloads) {
        lines.push(
          `| ${prototype.prototype} | ${workload.name} `
            + `| ${formatNumber(workload.median_execution_ms)} ms `
            + `| ${formatInteger(workload.median_maximum_node_wal_bytes)} `
            + `| ${formatInteger(workload.peak_maximum_node_wal_bytes)} `
            + `| ${workload.plan_shape_variants} |`,
        );
      }
    }
    lines.push('');
  }
  if (comparison.cross_scale_ratios) {
    lines.push('## 1m / 100k ratios', '');
    lines.push('| Prototype | Load ratio | Schema ratio | VACUUM ratio |');
    lines.push('| --- | ---: | ---: | ---: |');
    for (const item of comparison.cross_scale_ratios) {
      lines.push(
        `| ${item.prototype} | ${formatNumber(item.load_ms_ratio_1m_to_100k)}x `
          + `| ${formatNumber(item.schema_bytes_ratio_1m_to_100k)}x `
          + `| ${formatNumber(item.vacuum_duration_ratio_1m_to_100k)}x |`,
      );
    }
    lines.push('');
  }
  lines.push('## Manual ADR inputs still required', '');
  lines.push('- operational complexity and schema-evolution cost;');
  lines.push('- index-management and migration strategy;');
  lines.push('- acceptable trade-offs across latency, relation size, WAL and maintenance;');
  lines.push('- selected model and explicit rejection rationale for the alternatives.');
  lines.push('');
  return `${lines.join('\n')}\n`;
};

const { inputs, output } = parseArgs(process.argv.slice(2));
const scaleOrder = ['smoke', '100k', '1m'];
const scales = inputs
  .map(loadScale)
  .sort((left, right) => scaleOrder.indexOf(left.scale) - scaleOrder.indexOf(right.scale));
if (new Set(scales.map((scale) => scale.scale)).size !== scales.length) {
  fail('duplicate scale input');
}
const comparison = {
  generated_at: new Date().toISOString(),
  methodology: {
    first_run: 'first EXPLAIN ANALYZE repetition',
    warm_run: 'median of repetitions after the first; not a guaranteed OS cold-cache comparison',
    latency: 'milliseconds reported by PostgreSQL EXPLAIN ANALYZE',
    wal: 'maximum per-plan-node WAL metric recorded by the mutation harness',
    maintenance: 'pg_stat_user_tables estimates plus exact cardinality and relation sizes',
    automatic_winner_selection: false,
  },
  decision_ready: scales.some((scale) => scale.scale === '100k')
    && scales.some((scale) => scale.scale === '1m'),
  scales,
  cross_scale_ratios: buildCrossScaleRatios(scales),
};
mkdirSync(output, { recursive: true });
writeFileSync(
  path.join(output, 'comparison.json'),
  `${JSON.stringify(comparison, null, 2)}\n`,
);
writeFileSync(path.join(output, 'comparison.md'), renderMarkdown(comparison));
console.log(
  `[compare-index-storage-evidence] wrote ${path.join(output, 'comparison.json')} `
    + `and comparison.md; decision_ready=${comparison.decision_ready}`,
);
