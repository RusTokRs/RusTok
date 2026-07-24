#!/usr/bin/env node

import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

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

const requireSessionMetadata = (read, directory) => {
  const database = requireObject(read.database, `${directory} read.database`);
  for (const [field, expected] of canonicalSessionMetadata) {
    if (database[field] !== expected) {
      fail(`${directory} read.database.${field} must be ${expected}; got ${database[field]}`);
    }
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

const readReport = (directory) => {
  const filename = path.join(directory, 'read-report.json');
  if (!existsSync(filename)) fail(`missing evidence file: ${filename}`);
  try {
    return JSON.parse(readFileSync(filename, 'utf8'));
  } catch (error) {
    fail(`invalid JSON in ${filename}: ${error.message}`);
  }
};

export const validatePacketReadOrdering = (directory) => {
  const read = requireObject(readReport(directory), `${directory} read report`);
  requireSessionMetadata(read, directory);
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
  console.log(`${prefix} deterministic session metadata and executable terminal ordering verified for ${inputs.length} evidence packet(s)`);
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
