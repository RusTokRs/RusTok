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

const fail = (message) => {
  throw new Error(message);
};

const sameJson = (left, right) => JSON.stringify(left) === JSON.stringify(right);

const requireObject = (value, label) => {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    fail(`${label} must be an object`);
  }
  return value;
};

const requireExactNames = (items, expected, label, field = 'name') => {
  if (!Array.isArray(items)) fail(`${label} must be an array`);
  const names = items.map((item) => item?.[field]);
  if (new Set(names).size !== names.length) fail(`${label} contains duplicate entries`);
  if (!sameJson(names, expected)) {
    fail(`${label} mismatch: expected ${expected.join(', ')}, got ${names.join(', ')}`);
  }
};

export const requireTerminalReadOrdering = (sql, workloadName, label) => {
  if (typeof sql !== 'string' || sql.trim().length === 0) {
    fail(`${label}.sql must be a non-empty string`);
  }
  if (!readOrderMarkers.has(workloadName)) {
    fail(`${label} has no canonical ordering contract`);
  }
  const marker = readOrderMarkers.get(workloadName);
  if (marker !== null && !sql.trimEnd().endsWith(marker)) {
    fail(`${label}.sql must end with canonical ordering marker ${marker}`);
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
  console.log(`${prefix} terminal ordering verified for ${inputs.length} evidence packet(s)`);
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
