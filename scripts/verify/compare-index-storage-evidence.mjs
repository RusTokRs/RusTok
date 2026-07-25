#!/usr/bin/env node

import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

import { validatePacketReadOrdering } from './check-index-storage-read-ordering.mjs';

const prefix = '[compare-index-storage-evidence]';
const comparableDatabaseFields = [
  'server_version_num',
  'shared_buffers',
  'effective_cache_size',
  'work_mem',
  'random_page_cost',
  'jit',
  'standard_conforming_strings',
  'timezone',
  'date_style',
  'extra_float_digits',
];

const preflightArgs = (args) => {
  const inputs = [];
  let output = 'evidence/index-storage/comparison';
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === '--help' || argument === '-h') return null;
    if (argument === '--input' && args[index + 1] && !args[index + 1].startsWith('--')) {
      inputs.push(args[++index]);
    } else if (argument === '--output' && args[index + 1] && !args[index + 1].startsWith('--')) {
      output = args[++index];
    } else {
      return null;
    }
  }
  return { inputs, output };
};

const readJson = (filename, label) => {
  try {
    return JSON.parse(readFileSync(filename, 'utf8'));
  } catch (error) {
    throw new Error(`unable to read ${label} ${filename}: ${error.message}`);
  }
};

const requireObject = (value, label) => {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  return value;
};

const finalizeDatabaseSettingsContract = ({ inputs, output }) => {
  const packets = inputs.map((input) => {
    const provenance = requireObject(
      readJson(path.join(input, 'provenance.json'), 'evidence provenance'),
      `${input} provenance`,
    );
    const read = requireObject(
      readJson(path.join(input, 'read-report.json'), 'read evidence'),
      `${input} read report`,
    );
    const database = requireObject(read.database, `${input} read.database`);
    for (const field of comparableDatabaseFields) {
      if (typeof database[field] !== 'string' || database[field].length === 0) {
        throw new Error(`${input} read.database.${field} must be a non-empty string`);
      }
    }
    return { scale: provenance.scale, database };
  });

  const lower = packets.find((packet) => packet.scale === '100k');
  const upper = packets.find((packet) => packet.scale === '1m');
  if (lower && upper) {
    for (const field of comparableDatabaseFields) {
      if (lower.database[field] !== upper.database[field]) {
        throw new Error(`cross-scale database setting ${field} mismatch`);
      }
    }
  }

  const comparisonPath = path.join(output, 'comparison.json');
  const report = requireObject(readJson(comparisonPath, 'comparison report'), 'comparison report');
  const methodology = requireObject(report.methodology, 'comparison methodology');
  methodology.comparable_database_fields = comparableDatabaseFields;
  methodology.database_settings_source =
    'read-report.json database metadata observed from the active PostgreSQL benchmark session';
  writeFileSync(comparisonPath, `${JSON.stringify(report, null, 2)}\n`);

  const markdownPath = path.join(output, 'comparison.md');
  const lines = readFileSync(markdownPath, 'utf8').split('\n');
  const settingsLine = lines.findIndex((line) => line.startsWith('- Same PostgreSQL image/settings:'));
  if (settingsLine < 0) {
    throw new Error('comparison markdown is missing the PostgreSQL image/settings decision line');
  }
  const comparedFields = comparableDatabaseFields.map((field) => `\`${field}\``).join(', ');
  lines.splice(settingsLine + 1, 0, `- Compared PostgreSQL fields: ${comparedFields}`);
  writeFileSync(markdownPath, lines.join('\n'));
};

const main = async () => {
  const parsed = preflightArgs(process.argv.slice(2));
  if (parsed !== null) {
    for (const input of parsed.inputs) validatePacketReadOrdering(input);
  }
  await import('./compare-index-storage-evidence-core.mjs');
  if (parsed !== null) finalizeDatabaseSettingsContract(parsed);
};

try {
  await main();
} catch (error) {
  console.error(`${prefix} ${error.message}`);
  process.exitCode = 1;
}
