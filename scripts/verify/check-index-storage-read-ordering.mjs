#!/usr/bin/env node

import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { comparableDatabaseFields } from './index-storage-database-settings-contract.mjs';

const prefix = '[check-index-storage-read-ordering]';
const canonicalPrototypes = ['jsonb', 'typed_eav', 'hot_projection'];
const canonicalReadWorkloads = [
  'status_equality',
  'price_range_sort',
  'multi_value_tag',
  'two_hop_channel_filter',
  'keyset_page',
  'exact_count',
];
const readOrderMarkers = new Map([
  ['status_equality', 'ORDER BY entity_id LIMIT 100'],
  ['price_range_sort', 'ORDER BY price_minor, entity_id LIMIT 100'],
  ['multi_value_tag', 'ORDER BY entity_id LIMIT 100'],
  ['two_hop_channel_filter', 'ORDER BY entity_id LIMIT 100'],
  ['keyset_page', 'ORDER BY price_minor, entity_id LIMIT 100'],
  ['exact_count', null],
]);
const canonicalSessionMetadata = new Map([
  ['standard_conforming_strings', 'on'],
  ['timezone', 'UTC'],
  ['date_style', 'ISO, YMD'],
  ['extra_float_digits', '3'],
]);
const canonicalDatabaseMetadataFields = Object.freeze([
  'version',
  ...comparableDatabaseFields,
]);
const canonicalDatabaseMetadataFieldSet = [...canonicalDatabaseMetadataFields].sort();
const canonicalRunProvenanceFields = Object.freeze([
  'repository',
  'commit',
  'ref',
  'run_id',
  'run_attempt',
  'job',
  'runner_os',
  'runner_arch',
]);
const canonicalRunProvenanceFieldSet = [...canonicalRunProvenanceFields].sort();
const githubProvenanceEnvironment = new Map([
  ['repository', 'GITHUB_REPOSITORY'],
  ['commit', 'GITHUB_SHA'],
  ['ref', 'GITHUB_REF'],
  ['run_id', 'GITHUB_RUN_ID'],
  ['run_attempt', 'GITHUB_RUN_ATTEMPT'],
  ['job', 'GITHUB_JOB'],
  ['runner_os', 'RUNNER_OS'],
  ['runner_arch', 'RUNNER_ARCH'],
]);

const fail = (message) => {
  throw new Error(message);
};

const sameJson = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const maskSqlText = (text) => text.replace(/[^\r\n]/gu, ' ');
const identifierContinuation = /[A-Za-z0-9_$]/u;

const isEscapeStringQuote = (sql, quoteIndex) => {
  const prefixIndex = quoteIndex - 1;
  if (prefixIndex < 0 || (sql[prefixIndex] !== 'E' && sql[prefixIndex] !== 'e')) return false;
  const beforePrefix = sql[prefixIndex - 1];
  return beforePrefix === undefined || !identifierContinuation.test(beforePrefix);
};

const requireObject = (value, label) => {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    fail(`${label} must be an object`);
  }
  return value;
};

const requireCanonicalSessionMetadata = (database, label) => {
  for (const [field, expected] of canonicalSessionMetadata) {
    if (database[field] !== expected) {
      fail(`${label}.${field} must be ${expected}; got ${database[field]}`);
    }
  }
};

const requireDatabaseMetadata = (report, label) => {
  const database = requireObject(report.database, `${label}.database`);
  const actualFields = Object.keys(database).sort();
  if (!sameJson(actualFields, canonicalDatabaseMetadataFieldSet)) {
    fail(
      `${label}.database fields mismatch: expected ${canonicalDatabaseMetadataFields.join(', ')}, got ${actualFields.join(', ')}`,
    );
  }
  requireCanonicalSessionMetadata(database, `${label}.database`);
  return database;
};

const requireSessionMetadata = (read, directory) =>
  requireDatabaseMetadata(read, `${directory} read`);

const requireSameDatabaseMetadata = (expected, actual, label) => {
  for (const field of canonicalDatabaseMetadataFields) {
    if (actual[field] !== expected[field]) {
      fail(
        `${label}.database.${field} must match read-report.json database metadata; expected ${expected[field]}, got ${actual[field]}`,
      );
    }
  }
};

const readRunProvenance = (report, label) => {
  if (!Object.hasOwn(report, 'provenance')) {
    if (!Object.hasOwn(report, 'generated_at')) return null;
    fail(`${label}.provenance must be an object`);
  }
  const provenance = requireObject(report.provenance, `${label}.provenance`);
  const actualFields = Object.keys(provenance).sort();
  if (!sameJson(actualFields, canonicalRunProvenanceFieldSet)) {
    fail(
      `${label}.provenance fields mismatch: expected ${canonicalRunProvenanceFields.join(', ')}, got ${actualFields.join(', ')}`,
    );
  }
  for (const field of canonicalRunProvenanceFields) {
    const value = provenance[field];
    if (value !== null && (typeof value !== 'string' || value.length === 0)) {
      fail(`${label}.provenance.${field} must be a non-empty string or null`);
    }
  }
  return provenance;
};

const requireSameRunProvenance = (expected, actual, label) => {
  for (const field of canonicalRunProvenanceFields) {
    if (actual[field] !== expected[field]) {
      fail(
        `${label}.provenance.${field} must match read-report.json run provenance; expected ${expected[field]}, got ${actual[field]}`,
      );
    }
  }
};

const requireCurrentGitHubProvenance = (provenance, label) => {
  if (process.env.INDEX_BENCH_REQUIRE_GITHUB_PROVENANCE !== '1') return;
  for (const [field, environmentName] of githubProvenanceEnvironment) {
    const expected = process.env[environmentName];
    if (typeof expected !== 'string' || expected.length === 0) {
      fail(`${environmentName} is required when GitHub provenance is enforced`);
    }
    if (provenance[field] !== expected) {
      fail(`${label}.provenance.${field} must match current ${environmentName}`);
    }
  }
  if (!/^[0-9a-f]{40}$/iu.test(provenance.commit)) {
    fail(`${label}.provenance.commit must be a full Git SHA`);
  }
  if (!/^\d+$/u.test(provenance.run_id) || !/^\d+$/u.test(provenance.run_attempt)) {
    fail(`${label}.provenance run_id and run_attempt must be numeric strings`);
  }
};

const requireExactNames = (items, expected, label, field = 'name') => {
  if (!Array.isArray(items)) fail(`${label} must be an array`);
  const names = items.map((item) => item?.[field]);
  if (new Set(names).size !== names.length) fail(`${label} contains duplicate entries`);
  if (!sameJson(names, expected)) {
    fail(`${label} mismatch: expected ${expected.join(', ')}, got ${names.join(', ')}`);
  }
};

const executableSqlText = (sql, label) => {
  let output = '';
  let index = 0;
  while (index < sql.length) {
    if (sql.startsWith('--', index)) {
      const lineEnd = sql.indexOf('\n', index + 2);
      const end = lineEnd === -1 ? sql.length : lineEnd;
      output += maskSqlText(sql.slice(index, end));
      index = end;
      continue;
    }

    if (sql.startsWith('/*', index)) {
      const start = index;
      let depth = 1;
      index += 2;
      while (index < sql.length && depth > 0) {
        if (sql.startsWith('/*', index)) {
          depth += 1;
          index += 2;
        } else if (sql.startsWith('*/', index)) {
          depth -= 1;
          index += 2;
        } else {
          index += 1;
        }
      }
      if (depth !== 0) fail(`${label}.sql contains an unterminated block comment`);
      output += maskSqlText(sql.slice(start, index));
      continue;
    }

    const quote = sql[index];
    if (quote === "'" || quote === '"') {
      const start = index;
      const escapeString = quote === "'" && isEscapeStringQuote(sql, index);
      index += 1;
      let closed = false;
      while (index < sql.length) {
        if (escapeString && sql[index] === '\\') {
          if (index + 1 >= sql.length) {
            fail(`${label}.sql contains an unterminated escape string literal`);
          }
          index += 2;
          continue;
        }
        if (sql[index] === quote) {
          if (sql[index + 1] === quote) {
            index += 2;
            continue;
          }
          index += 1;
          closed = true;
          break;
        }
        index += 1;
      }
      if (!closed) {
        const kind = quote === "'"
          ? (escapeString ? 'escape string literal' : 'string literal')
          : 'quoted identifier';
        fail(`${label}.sql contains an unterminated ${kind}`);
      }
      output += maskSqlText(sql.slice(start, index));
      continue;
    }

    if (sql[index] === '$') {
      const delimiter = sql.slice(index).match(/^(?:\$\$|\$[A-Za-z_][A-Za-z0-9_]*\$)/u)?.[0];
      if (delimiter) {
        const start = index;
        const close = sql.indexOf(delimiter, index + delimiter.length);
        if (close === -1) fail(`${label}.sql contains an unterminated dollar-quoted string`);
        index = close + delimiter.length;
        output += maskSqlText(sql.slice(start, index));
        continue;
      }
    }

    output += sql[index];
    index += 1;
  }
  return output;
};

export const requireTerminalReadOrdering = (sql, workloadName, label) => {
  if (typeof sql !== 'string' || sql.trim().length === 0) {
    fail(`${label}.sql must be a non-empty string`);
  }
  if (!readOrderMarkers.has(workloadName)) {
    fail(`${label} has no canonical ordering contract`);
  }
  const marker = readOrderMarkers.get(workloadName);
  const executableSql = executableSqlText(sql, label);
  if (marker !== null && !executableSql.trimEnd().endsWith(marker)) {
    fail(`${label}.sql must end with canonical ordering marker ${marker} in executable SQL`);
  }
};

const readReport = (directory, filename = 'read-report.json') => {
  const reportPath = path.join(directory, filename);
  if (!existsSync(reportPath)) fail(`missing evidence file: ${reportPath}`);
  try {
    return JSON.parse(readFileSync(reportPath, 'utf8'));
  } catch (error) {
    fail(`invalid JSON in ${reportPath}: ${error.message}`);
  }
};

const requireExistingPacketProvenance = (directory, expected) => {
  if (expected === null) return;
  const filename = 'provenance.json';
  if (!existsSync(path.join(directory, filename))) return;
  const packet = requireObject(readReport(directory, filename), `${directory} ${filename}`);
  const actual = Object.fromEntries(
    canonicalRunProvenanceFields.map((field) => [field, packet[field] ?? null]),
  );
  requireSameRunProvenance(expected, actual, `${directory} ${filename}`);
};

export const validatePacketReadOrdering = (directory) => {
  const read = requireObject(readReport(directory), `${directory} read report`);
  const readDatabase = requireSessionMetadata(read, directory);
  const readProvenance = readRunProvenance(read, `${directory} read`);
  if (readProvenance !== null) requireCurrentGitHubProvenance(readProvenance, `${directory} read`);
  requireExactNames(read.source_workloads, canonicalReadWorkloads, `${directory} source workload order`);
  for (const workload of read.source_workloads) {
    requireObject(workload, `${directory} source/${workload?.name ?? 'unknown'}`);
    requireTerminalReadOrdering(workload.sql, workload.name, `${directory} source/${workload.name}`);
  }

  requireExactNames(read.prototypes, canonicalPrototypes, `${directory} prototype order`, 'prototype');
  for (const prototype of read.prototypes) {
    requireObject(prototype, `${directory} prototype/${prototype?.prototype ?? 'unknown'}`);
    requireExactNames(
      prototype.workloads,
      canonicalReadWorkloads,
      `${directory} ${prototype.prototype} read workload order`,
    );
    for (const workload of prototype.workloads) {
      requireObject(workload, `${directory} ${prototype.prototype}/${workload?.name ?? 'unknown'}`);
      requireTerminalReadOrdering(
        workload.sql,
        workload.name,
        `${directory} ${prototype.prototype}/${workload.name}`,
      );
    }
  }

  for (const filename of ['mutation-report.json', 'maintenance-report.json']) {
    const report = requireObject(readReport(directory, filename), `${directory} ${filename}`);
    const database = requireDatabaseMetadata(report, `${directory} ${filename}`);
    requireSameDatabaseMetadata(readDatabase, database, `${directory} ${filename}`);
    const provenance = readRunProvenance(report, `${directory} ${filename}`);
    if ((readProvenance === null) !== (provenance === null)) {
      fail(`${directory} reports must either all contain run provenance or all be incomplete core fixtures`);
    }
    if (readProvenance !== null && provenance !== null) {
      requireSameRunProvenance(readProvenance, provenance, `${directory} ${filename}`);
      requireCurrentGitHubProvenance(provenance, `${directory} ${filename}`);
    }
  }
  requireExistingPacketProvenance(directory, readProvenance);
};

const usage = () => {
  console.log('Usage: node scripts/verify/check-index-storage-read-ordering.mjs --input <evidence-dir> [--input <evidence-dir>]');
};

const parseArgs = () => {
  const inputs = [];
  const args = process.argv.slice(2);
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === '--help' || argument === '-h') {
      if (args.length !== 1) fail('help must be the only argument');
      usage();
      return null;
    }
    if (argument !== '--input' || !args[index + 1] || args[index + 1].startsWith('--')) {
      fail(`unknown or incomplete argument: ${argument}`);
    }
    inputs.push(args[++index]);
  }
  if (inputs.length === 0) fail('at least one --input evidence directory is required');
  return inputs;
};

const main = () => {
  const inputs = parseArgs();
  if (inputs === null) return;
  for (const input of inputs) validatePacketReadOrdering(input);
  console.log(`${prefix} benchmark run provenance, deterministic session metadata, and executable terminal ordering verified across read, mutation, and maintenance reports for ${inputs.length} evidence packet(s)`);
};

const isMain = process.argv[1]
  && path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url));
if (isMain) {
  try {
    main();
  } catch (error) {
    console.error(`${prefix} ${error.message}`);
    process.exitCode = 1;
  }
}
