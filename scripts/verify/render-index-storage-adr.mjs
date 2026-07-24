#!/usr/bin/env node

import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const prefix = '[render-index-storage-adr]';
const fail = (message) => {
  console.error(`${prefix} ${message}`);
  process.exit(1);
};

const prototypes = ['jsonb', 'typed_eav', 'hot_projection'];
const requiredDecisionFlags = [
  'required_scales_present',
  'same_packet_contract_version',
  'same_result_digest_contract',
  'same_repository',
  'same_commit',
  'same_postgres_image',
  'same_repetitions',
  'same_churn_cycles',
  'same_database_settings',
  'same_dataset_shape',
  'same_source_oracle_shape',
  'same_report_shape',
  'same_mutation_effect_contract',
];

const parseArgs = () => {
  const values = new Map();
  const args = process.argv.slice(2);
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === '--help' || argument === '-h') {
      console.log(
        'Usage: node scripts/verify/render-index-storage-adr.mjs '
        + '--comparison <comparison.json> --decision <decision.json> --output <adr.md>',
      );
      process.exit(0);
    }
    if (!argument.startsWith('--') || !args[index + 1]) {
      fail(`unknown or incomplete argument: ${argument}`);
    }
    values.set(argument, args[++index]);
  }
  for (const argument of ['--comparison', '--decision', '--output']) {
    if (!values.has(argument)) fail(`${argument} is required`);
  }
  return {
    comparison: values.get('--comparison'),
    decision: values.get('--decision'),
    output: values.get('--output'),
  };
};

const readJson = (filename, label) => {
  if (!existsSync(filename)) fail(`missing ${label}: ${filename}`);
  try {
    return JSON.parse(readFileSync(filename, 'utf8'));
  } catch (error) {
    fail(`invalid JSON in ${label} ${filename}: ${error.message}`);
  }
};

const requireObject = (value, label) => {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    fail(`${label} must be an object`);
  }
  return value;
};

const requireArray = (value, label) => {
  if (!Array.isArray(value)) fail(`${label} must be an array`);
  return value;
};

const requireNonEmptyString = (value, label) => {
  if (typeof value !== 'string' || value.trim().length === 0) {
    fail(`${label} must be a non-empty string`);
  }
  return value.trim();
};

const requireNonNegativeNumber = (value, label) => {
  if (!Number.isFinite(value) || value < 0) fail(`${label} must be a non-negative number`);
  return value;
};

const requirePositiveInteger = (value, label) => {
  if (!Number.isInteger(value) || value <= 0) fail(`${label} must be a positive integer`);
  return value;
};

const requireDate = (value, label) => {
  requireNonEmptyString(value, label);
  if (!/^\d{4}-\d{2}-\d{2}$/u.test(value) || !Number.isFinite(Date.parse(`${value}T00:00:00Z`))) {
    fail(`${label} must be an ISO calendar date`);
  }
};

const sameJson = (left, right) => JSON.stringify(left) === JSON.stringify(right);

const requireExactNames = (items, expected, label) => {
  requireArray(items, label);
  const names = items.map((item) => item?.name ?? item?.prototype);
  if (new Set(names).size !== names.length) fail(`${label} contains duplicates`);
  if (!sameJson(names, expected)) {
    fail(`${label} mismatch: expected ${expected.join(', ')}, got ${names.join(', ')}`);
  }
};

const findScale = (comparison, scale) => {
  const matches = comparison.scales.filter((item) => item.scale === scale);
  if (matches.length !== 1) fail(`comparison must contain exactly one ${scale} evidence entry`);
  return matches[0];
};

const findPrototype = (scale, section, prototype) => {
  const result = scale[section]?.find((item) => item.prototype === prototype);
  if (!result) fail(`${scale.scale} ${section} evidence is missing ${prototype}`);
  return result;
};

const findWorkload = (prototype, workloadName) => {
  const result = prototype.workloads?.find((item) => item.name === workloadName);
  if (!result) fail(`${prototype.prototype} is missing workload ${workloadName}`);
  return result;
};

const number = (value, digits = 2) => value.toFixed(digits);
const integer = (value) => value === null
  ? 'n/a'
  : Math.round(value).toLocaleString('en-US');
const bytes = (value) => {
  const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
  let amount = value;
  let index = 0;
  while (Math.abs(amount) >= 1024 && index < units.length - 1) {
    amount /= 1024;
    index += 1;
  }
  return `${amount.toFixed(index === 0 ? 0 : 2)} ${units[index]}`;
};

const validateScaleShape = (scale) => {
  requireObject(scale, `${scale?.scale ?? 'unknown'} evidence`);
  const provenance = requireObject(scale.provenance, `${scale.scale} provenance`);
  requirePositiveInteger(provenance.packet_contract_version, `${scale.scale} packet contract version`);
  requireNonEmptyString(provenance.result_digest_contract, `${scale.scale} result digest contract`);
  requireNonEmptyString(provenance.postgres_image, `${scale.scale} PostgreSQL image`);
  requireExactNames(scale.read, prototypes, `${scale.scale} read prototype order`);
  requireExactNames(scale.mutation, prototypes, `${scale.scale} mutation prototype order`);
  requireExactNames(scale.maintenance, prototypes, `${scale.scale} maintenance prototype order`);

  const readNames = new Map();
  const mutationNames = new Map();
  for (const prototype of prototypes) {
    const read = findPrototype(scale, 'read', prototype);
    requireNonNegativeNumber(read.schema_bytes, `${scale.scale}/${prototype}.schema_bytes`);
    if (read.schema_bytes === 0) fail(`${scale.scale}/${prototype}.schema_bytes must be positive`);
    requireArray(read.workloads, `${scale.scale}/${prototype} read workloads`);
    if (read.workloads.length === 0) fail(`${scale.scale}/${prototype} read workloads must not be empty`);
    const names = read.workloads.map((item) => item?.name);
    if (new Set(names).size !== names.length) fail(`${scale.scale}/${prototype} read workloads contain duplicates`);
    readNames.set(prototype, names);

    const mutation = findPrototype(scale, 'mutation', prototype);
    requireArray(mutation.workloads, `${scale.scale}/${prototype} mutation workloads`);
    if (mutation.workloads.length === 0) fail(`${scale.scale}/${prototype} mutation workloads must not be empty`);
    const mutationWorkloadNames = mutation.workloads.map((item) => item?.name);
    if (new Set(mutationWorkloadNames).size !== mutationWorkloadNames.length) {
      fail(`${scale.scale}/${prototype} mutation workloads contain duplicates`);
    }
    mutationNames.set(prototype, mutationWorkloadNames);

    const maintenance = findPrototype(scale, 'maintenance', prototype);
    const afterChurn = requireObject(
      maintenance.after_churn,
      `${scale.scale}/${prototype} maintenance.after_churn`,
    );
    if (prototype === 'typed_eav') {
      requireNonNegativeNumber(afterChurn.field_rows, `${scale.scale}/${prototype}.field_rows`);
    } else if (afterChurn.field_rows !== null) {
      fail(`${scale.scale}/${prototype}.field_rows must be null`);
    }
    requireNonNegativeNumber(
      maintenance.churn_growth_percent,
      `${scale.scale}/${prototype}.churn_growth_percent`,
    );
    requireNonNegativeNumber(
      maintenance.vacuum_duration_ms,
      `${scale.scale}/${prototype}.vacuum_duration_ms`,
    );
  }

  for (const prototype of prototypes.slice(1)) {
    if (!sameJson(readNames.get(prototype), readNames.get(prototypes[0]))) {
      fail(`${scale.scale} read workload order differs across prototypes`);
    }
    if (!sameJson(mutationNames.get(prototype), mutationNames.get(prototypes[0]))) {
      fail(`${scale.scale} mutation workload order differs across prototypes`);
    }
  }
};

const validateComparison = (comparison) => {
  requireObject(comparison, 'comparison');
  if (comparison.decision_ready !== true) fail('comparison is not decision-ready');
  const decisionContract = requireObject(comparison.decision_contract, 'comparison.decision_contract');
  for (const field of requiredDecisionFlags) {
    if (decisionContract[field] !== true) fail(`comparison decision contract ${field} is not satisfied`);
  }
  if (comparison.methodology?.automatic_winner_selection !== false) {
    fail('comparison must explicitly disable automatic winner selection');
  }
  requireArray(comparison.scales, 'comparison.scales');
  const scaleNames = comparison.scales.map((item) => item?.scale);
  if (new Set(scaleNames).size !== scaleNames.length) fail('comparison contains duplicate scales');
  const lower = findScale(comparison, '100k');
  const upper = findScale(comparison, '1m');
  validateScaleShape(lower);
  validateScaleShape(upper);

  const lowerCommit = requireNonEmptyString(lower.provenance.commit, '100k provenance.commit');
  const upperCommit = requireNonEmptyString(upper.provenance.commit, '1m provenance.commit');
  if (lowerCommit !== upperCommit) fail('100k and 1m evidence commits differ');
  if (!/^[0-9a-f]{40}$/iu.test(lowerCommit)) fail('comparison commit must be a full Git SHA');
  for (const field of ['packet_contract_version', 'result_digest_contract', 'postgres_image']) {
    if (lower.provenance[field] !== upper.provenance[field]) {
      fail(`100k and 1m provenance ${field} differ`);
    }
  }

  const crossScale = requireObject(comparison.cross_scale_ratios, 'comparison.cross_scale_ratios');
  requireExactNames(crossScale.prototypes, prototypes, 'cross-scale prototype order');
  return { lower, upper, commit: lowerCommit, crossScale };
};

const validateDecision = (decision, comparisonCommit) => {
  requireObject(decision, 'decision');
  if (!['proposed', 'accepted'].includes(decision.status)) {
    fail('decision.status must be proposed or accepted');
  }
  requireDate(decision.decision_date, 'decision.decision_date');
  requireNonEmptyString(decision.owner, 'decision.owner');
  if (!prototypes.includes(decision.selected_prototype)) {
    fail(`decision.selected_prototype must be one of ${prototypes.join(', ')}`);
  }
  if (decision.comparison_commit !== comparisonCommit) {
    fail('decision.comparison_commit must match the evidence comparison commit');
  }
  for (const field of [
    'selection_rationale',
    'operational_tradeoffs',
    'migration_strategy',
    'rollback_strategy',
  ]) {
    requireNonEmptyString(decision[field], `decision.${field}`);
  }
  const rejections = requireObject(decision.rejection_rationales, 'decision.rejection_rationales');
  const rejected = prototypes.filter((prototype) => prototype !== decision.selected_prototype);
  const keys = Object.keys(rejections).sort();
  if (!sameJson(keys, [...rejected].sort())) {
    fail(`decision.rejection_rationales must contain exactly ${rejected.join(', ')}`);
  }
  for (const prototype of rejected) {
    requireNonEmptyString(rejections[prototype], `decision.rejection_rationales.${prototype}`);
  }
};

const storageRows = (lower, upper, crossScale) => prototypes.map((prototype) => {
  const read100k = findPrototype(lower, 'read', prototype);
  const read1m = findPrototype(upper, 'read', prototype);
  const maintenance100k = findPrototype(lower, 'maintenance', prototype);
  const maintenance1m = findPrototype(upper, 'maintenance', prototype);
  const ratio = crossScale.prototypes.find((item) => item.prototype === prototype);
  requireNonNegativeNumber(ratio?.schema_bytes_ratio_1m_to_100k, `${prototype} schema growth ratio`);
  return {
    prototype,
    schema100k: requireNonNegativeNumber(read100k.schema_bytes, `${prototype} 100k schema bytes`),
    schema1m: requireNonNegativeNumber(read1m.schema_bytes, `${prototype} 1m schema bytes`),
    schemaRatio: ratio.schema_bytes_ratio_1m_to_100k,
    fields100k: maintenance100k.after_churn.field_rows,
    fields1m: maintenance1m.after_churn.field_rows,
    churn100k: requireNonNegativeNumber(maintenance100k.churn_growth_percent, `${prototype} 100k churn growth`),
    churn1m: requireNonNegativeNumber(maintenance1m.churn_growth_percent, `${prototype} 1m churn growth`),
    vacuum100k: requireNonNegativeNumber(maintenance100k.vacuum_duration_ms, `${prototype} 100k VACUUM`),
    vacuum1m: requireNonNegativeNumber(maintenance1m.vacuum_duration_ms, `${prototype} 1m VACUUM`),
  };
});

const readRows = (lower, upper, crossScale) => {
  const rows = [];
  for (const prototype of prototypes) {
    const read100k = findPrototype(lower, 'read', prototype);
    const read1m = findPrototype(upper, 'read', prototype);
    const names100k = read100k.workloads.map((item) => item.name);
    const names1m = read1m.workloads.map((item) => item.name);
    if (!sameJson(names100k, names1m)) fail(`${prototype} read workload order differs across scales`);
    const ratioPrototype = crossScale.prototypes.find((item) => item.prototype === prototype);
    requireExactNames(ratioPrototype?.read_workloads, names100k, `${prototype} read ratio workload order`);
    for (const workload100k of read100k.workloads) {
      const workload1m = findWorkload(read1m, workload100k.name);
      const ratio = ratioPrototype.read_workloads.find((item) => item.name === workload100k.name);
      rows.push({
        prototype,
        workload: workload100k.name,
        warm100k: requireNonNegativeNumber(workload100k.warm_median_execution_ms, `${prototype}/${workload100k.name} 100k warm median`),
        warm1m: requireNonNegativeNumber(workload1m.warm_median_execution_ms, `${prototype}/${workload100k.name} 1m warm median`),
        ratio: requireNonNegativeNumber(ratio?.warm_execution_ratio_1m_to_100k, `${prototype}/${workload100k.name} read growth ratio`),
        plans100k: requirePositiveInteger(workload100k.plan_shape_variants, `${prototype}/${workload100k.name} 100k plan shapes`),
        plans1m: requirePositiveInteger(workload1m.plan_shape_variants, `${prototype}/${workload100k.name} 1m plan shapes`),
      });
    }
  }
  return rows;
};

const mutationRows = (lower, upper, crossScale) => {
  const rows = [];
  for (const prototype of prototypes) {
    const mutation100k = findPrototype(lower, 'mutation', prototype);
    const mutation1m = findPrototype(upper, 'mutation', prototype);
    const names100k = mutation100k.workloads.map((item) => item.name);
    const names1m = mutation1m.workloads.map((item) => item.name);
    if (!sameJson(names100k, names1m)) fail(`${prototype} mutation workload order differs across scales`);
    const ratioPrototype = crossScale.prototypes.find((item) => item.prototype === prototype);
    requireExactNames(
      ratioPrototype?.mutation_workloads,
      names100k,
      `${prototype} mutation ratio workload order`,
    );
    for (const workload100k of mutation100k.workloads) {
      const workload1m = findWorkload(mutation1m, workload100k.name);
      const ratio = ratioPrototype.mutation_workloads.find((item) => item.name === workload100k.name);
      rows.push({
        prototype,
        workload: workload100k.name,
        execution100k: requireNonNegativeNumber(workload100k.median_execution_ms, `${prototype}/${workload100k.name} 100k mutation median`),
        execution1m: requireNonNegativeNumber(workload1m.median_execution_ms, `${prototype}/${workload100k.name} 1m mutation median`),
        executionRatio: requireNonNegativeNumber(ratio?.execution_ratio_1m_to_100k, `${prototype}/${workload100k.name} mutation growth ratio`),
        wal100k: requireNonNegativeNumber(workload100k.median_maximum_node_wal_bytes, `${prototype}/${workload100k.name} 100k WAL`),
        wal1m: requireNonNegativeNumber(workload1m.median_maximum_node_wal_bytes, `${prototype}/${workload100k.name} 1m WAL`),
        walRatio: requireNonNegativeNumber(ratio?.wal_bytes_ratio_1m_to_100k, `${prototype}/${workload100k.name} WAL growth ratio`),
      });
    }
  }
  return rows;
};

const render = (comparison, decision) => {
  const { lower, upper, commit, crossScale } = validateComparison(comparison);
  validateDecision(decision, commit);
  const storage = storageRows(lower, upper, crossScale);
  const reads = readRows(lower, upper, crossScale);
  const mutations = mutationRows(lower, upper, crossScale);
  const rejected = prototypes.filter((prototype) => prototype !== decision.selected_prototype);
  const lines = [
    '# ADR: Index PostgreSQL storage model',
    '',
    `- Status: **${decision.status}**`,
    `- Decision date: **${decision.decision_date}**`,
    `- Owner: **${decision.owner}**`,
    `- Evidence commit: \`${commit}\``,
    `- Packet contract: \`v${lower.provenance.packet_contract_version}\``,
    `- Result digest contract: \`${lower.provenance.result_digest_contract}\``,
    `- PostgreSQL image: \`${lower.provenance.postgres_image}\``,
    '',
    '## Context',
    '',
    'The Index module evaluated JSONB, typed EAV, and hot projection storage using same-commit 100k and 1m PostgreSQL evidence. Candidate query results were checked against the normalized source oracle, and the comparison explicitly disabled automatic winner selection.',
    '',
    '## Decision',
    '',
    `Use **${decision.selected_prototype}** as the PostgreSQL persistence model for the next Index storage milestone.`,
    '',
    '## Rationale',
    '',
    decision.selection_rationale.trim(),
    '',
    '## Storage and maintenance evidence',
    '',
    '| Prototype | 100k schema | 1m schema | Growth | EAV fields 100k / 1m | Churn growth 100k / 1m | VACUUM 100k / 1m |',
    '| --- | ---: | ---: | ---: | ---: | ---: | ---: |',
  ];
  for (const row of storage) {
    lines.push(`| ${row.prototype} | ${bytes(row.schema100k)} | ${bytes(row.schema1m)} | ${number(row.schemaRatio)}x | ${integer(row.fields100k)} / ${integer(row.fields1m)} | ${number(row.churn100k)}% / ${number(row.churn1m)}% | ${number(row.vacuum100k, 0)} ms / ${number(row.vacuum1m, 0)} ms |`);
  }
  lines.push(
    '',
    '## Read/query evidence',
    '',
    '| Prototype | Workload | Warm median 100k | Warm median 1m | Growth | Plan shapes 100k / 1m |',
    '| --- | --- | ---: | ---: | ---: | ---: |',
  );
  for (const row of reads) {
    lines.push(`| ${row.prototype} | ${row.workload} | ${number(row.warm100k)} ms | ${number(row.warm1m)} ms | ${number(row.ratio)}x | ${integer(row.plans100k)} / ${integer(row.plans1m)} |`);
  }
  lines.push(
    '',
    '## Mutation and WAL evidence',
    '',
    '| Prototype | Workload | Median execution 100k / 1m | Growth | Median WAL 100k / 1m | WAL growth |',
    '| --- | --- | ---: | ---: | ---: | ---: |',
  );
  for (const row of mutations) {
    lines.push(`| ${row.prototype} | ${row.workload} | ${number(row.execution100k)} ms / ${number(row.execution1m)} ms | ${number(row.executionRatio)}x | ${bytes(row.wal100k)} / ${bytes(row.wal1m)} | ${number(row.walRatio)}x |`);
  }
  lines.push('', '## Rejected alternatives', '');
  for (const prototype of rejected) {
    lines.push(`### ${prototype}`, '', decision.rejection_rationales[prototype].trim(), '');
  }
  lines.push(
    '## Operational trade-offs',
    '',
    decision.operational_tradeoffs.trim(),
    '',
    '## Migration strategy',
    '',
    decision.migration_strategy.trim(),
    '',
    '## Rollback strategy',
    '',
    decision.rollback_strategy.trim(),
    '',
    '## Evidence limitations',
    '',
    '- The first repetition is only a first-run signal, not a guaranteed operating-system cold-cache measurement.',
    '- The benchmark evidence does not replace production observability, migration rehearsal, or failure-mode testing.',
    '- This ADR records a manual decision; the renderer does not infer or rank a winning prototype.',
    '',
  );
  return `${lines.join('\n')}\n`;
};

const args = parseArgs();
const comparison = readJson(args.comparison, 'comparison');
const decision = readJson(args.decision, 'decision');
const markdown = render(comparison, decision);
const parent = path.dirname(args.output);
if (parent && parent !== '.') mkdirSync(parent, { recursive: true });
writeFileSync(args.output, markdown);
console.log(`${prefix} wrote ${args.output}`);
