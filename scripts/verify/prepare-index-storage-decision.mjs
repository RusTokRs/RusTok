#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const prefix = '[prepare-index-storage-decision]';
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
const placeholderPrefix = 'TODO(index-storage-decision):';

const fail = (message) => {
  console.error(`${prefix} ${message}`);
  process.exit(1);
};

const usage = () => {
  console.log(
    'Usage: node scripts/verify/prepare-index-storage-decision.mjs '
    + '--comparison <comparison.json> --selected <jsonb|typed_eav|hot_projection> '
    + '--owner <owner> --date <YYYY-MM-DD> --output <decision.json> [--force]',
  );
};

const parseArgs = () => {
  const values = new Map();
  let force = false;
  const args = process.argv.slice(2);
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === '--help' || argument === '-h') {
      usage();
      process.exit(0);
    }
    if (argument === '--force') {
      if (force) fail('--force was provided more than once');
      force = true;
      continue;
    }
    if (!argument.startsWith('--') || !args[index + 1] || args[index + 1].startsWith('--')) {
      fail(`unknown or incomplete argument: ${argument}`);
    }
    if (values.has(argument)) fail(`${argument} was provided more than once`);
    values.set(argument, args[++index]);
  }
  for (const argument of ['--comparison', '--selected', '--owner', '--date', '--output']) {
    if (!values.has(argument)) fail(`${argument} is required`);
  }
  return {
    comparison: values.get('--comparison'),
    selected: values.get('--selected'),
    owner: values.get('--owner'),
    date: values.get('--date'),
    output: values.get('--output'),
    force,
  };
};

const requireObject = (value, label) => {
  if (!value || typeof value !== 'object' || Array.isArray(value)) fail(`${label} must be an object`);
  return value;
};

const requireDate = (value) => {
  if (!/^\d{4}-\d{2}-\d{2}$/u.test(value)) fail('--date must be an ISO calendar date');
  const parsed = new Date(`${value}T00:00:00Z`);
  if (!Number.isFinite(parsed.valueOf()) || parsed.toISOString().slice(0, 10) !== value) {
    fail('--date must be a real ISO calendar date');
  }
};

const readComparison = (filename) => {
  if (!existsSync(filename)) fail(`missing comparison: ${filename}`);
  const bytes = readFileSync(filename);
  try {
    return {
      comparison: JSON.parse(bytes.toString('utf8')),
      sha256: createHash('sha256').update(bytes).digest('hex'),
    };
  } catch (error) {
    fail(`invalid JSON in comparison ${filename}: ${error.message}`);
  }
};

const comparisonCommit = (comparison) => {
  requireObject(comparison, 'comparison');
  if (comparison.decision_ready !== true) fail('comparison is not decision-ready');
  if (comparison.methodology?.automatic_winner_selection !== false) {
    fail('comparison must explicitly disable automatic winner selection');
  }
  const decisionContract = requireObject(comparison.decision_contract, 'comparison.decision_contract');
  for (const field of requiredDecisionFlags) {
    if (decisionContract[field] !== true) fail(`comparison decision contract ${field} is not satisfied`);
  }
  if (!Array.isArray(comparison.scales) || comparison.scales.length !== 2) {
    fail('comparison must contain exactly the 100k and 1m scales');
  }
  const scale = (name) => {
    const matches = comparison.scales.filter((item) => item?.scale === name);
    if (matches.length !== 1) fail(`comparison must contain exactly one ${name} evidence entry`);
    return matches[0];
  };
  const lowerCommit = scale('100k').provenance?.commit;
  const upperCommit = scale('1m').provenance?.commit;
  if (typeof lowerCommit !== 'string' || !/^[0-9a-f]{40}$/iu.test(lowerCommit)) {
    fail('comparison commit must be a full Git SHA');
  }
  if (lowerCommit !== upperCommit) fail('100k and 1m evidence commits differ');
  return lowerCommit;
};

const args = parseArgs();
if (!prototypes.includes(args.selected)) {
  fail(`--selected must be one of ${prototypes.join(', ')}`);
}
if (args.owner.trim().length === 0) fail('--owner must be non-empty');
requireDate(args.date);
if (path.resolve(args.output) === path.resolve(args.comparison)) {
  fail('--output must not overwrite the comparison input');
}
if (existsSync(args.output) && !args.force) {
  fail(`refusing to overwrite existing decision without --force: ${args.output}`);
}

const { comparison, sha256 } = readComparison(args.comparison);
const commit = comparisonCommit(comparison);
const rejected = prototypes.filter((prototype) => prototype !== args.selected);
const decision = {
  status: 'proposed',
  decision_date: args.date,
  owner: args.owner.trim(),
  comparison_commit: commit,
  comparison_sha256: sha256,
  selected_prototype: args.selected,
  selection_rationale: `${placeholderPrefix} explain why ${args.selected} is preferred using measured and operational evidence.`,
  rejection_rationales: Object.fromEntries(rejected.map((prototype) => [
    prototype,
    `${placeholderPrefix} explain why ${prototype} was not selected.`,
  ])),
  operational_tradeoffs: `${placeholderPrefix} document indexing, schema evolution, relation growth, WAL, VACUUM, and observability implications.`,
  migration_strategy: `${placeholderPrefix} document table creation, backfill, parity verification, and persistence-port cutover.`,
  rollback_strategy: `${placeholderPrefix} document how the previous persistence path remains recoverable until cutover verification completes.`,
};

const parent = path.dirname(args.output);
if (parent && parent !== '.') mkdirSync(parent, { recursive: true });
writeFileSync(args.output, `${JSON.stringify(decision, null, 2)}\n`, 'utf8');
console.log(`${prefix} wrote ${args.output}`);
