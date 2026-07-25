#!/usr/bin/env node

import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import { validatePacketReadOrdering } from './check-index-storage-read-ordering.mjs';
import {
  comparableDatabaseFields,
  databaseSettingsSource,
} from './index-storage-database-settings-contract.mjs';

const prefix = '[compare-index-storage-evidence]';
const corePath = fileURLToPath(new URL('./compare-index-storage-evidence-core.mjs', import.meta.url));
const scriptPath = fileURLToPath(import.meta.url);

const preflightArgs = (args) => {
  const inputs = [];
  let output = 'evidence/index-storage/comparison';
  let outputProvided = false;
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === '--help' || argument === '-h') {
      if (args.length !== 1) throw new Error('help must be the only argument');
      return null;
    }
    if (argument === '--input' && args[index + 1] && !args[index + 1].startsWith('--')) {
      inputs.push(args[++index]);
    } else if (argument === '--output' && args[index + 1] && !args[index + 1].startsWith('--')) {
      if (outputProvided) throw new Error('--output was provided more than once');
      output = args[++index];
      outputProvided = true;
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
  methodology.database_settings_source = databaseSettingsSource;
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

const forwardOutput = (stream, value) => {
  if (typeof value === 'string' && value.length !== 0) stream.write(value);
};

const runCore = ({ args, spawn = spawnSync, stdout = process.stdout, stderr = process.stderr }) => {
  const result = spawn(process.execPath, [corePath, ...args], {
    encoding: 'utf8',
    env: process.env,
  });
  forwardOutput(stdout, result.stdout);
  forwardOutput(stderr, result.stderr);
  if (result.error) throw result.error;
  if (result.signal) throw new Error(`comparator core terminated by signal ${result.signal}`);
  return result.status ?? 1;
};

export const runComparatorCoreWithAtomicComparison = ({
  inputs,
  output,
  spawn = spawnSync,
  finalizeComparison = finalizeDatabaseSettingsContract,
  stdout = process.stdout,
  stderr = process.stderr,
}) => {
  const outputJson = path.join(output, 'comparison.json');
  const outputMarkdown = path.join(output, 'comparison.md');
  rmSync(outputJson, { force: true });
  mkdirSync(output, { recursive: true });
  const stagingRoot = mkdtempSync(path.join(output, '.comparison-staging-'));
  let published = false;
  try {
    const args = inputs.flatMap((input) => ['--input', input]);
    args.push('--output', stagingRoot);
    const status = runCore({ args, spawn, stdout, stderr });
    if (status !== 0) return status;

    finalizeComparison({ inputs, output: stagingRoot });
    const stagedJson = path.join(stagingRoot, 'comparison.json');
    const stagedMarkdown = path.join(stagingRoot, 'comparison.md');
    if (!existsSync(stagedJson) || !existsSync(stagedMarkdown)) {
      throw new Error('comparator core exited successfully without complete comparison outputs');
    }

    renameSync(stagedMarkdown, outputMarkdown);
    renameSync(stagedJson, outputJson);
    published = true;
    return 0;
  } finally {
    try {
      rmSync(stagingRoot, { recursive: true, force: true });
    } catch (error) {
      if (!published) throw error;
      forwardOutput(stderr, `${prefix} unable to remove staging directory: ${error.message}\n`);
    }
  }
};

const main = () => {
  const args = process.argv.slice(2);
  const parsed = preflightArgs(args);
  if (parsed === null) {
    const status = runCore({ args });
    if (status !== 0) process.exitCode = status;
    return;
  }

  rmSync(path.join(parsed.output, 'comparison.json'), { force: true });
  for (const input of parsed.inputs) validatePacketReadOrdering(input);
  const status = runComparatorCoreWithAtomicComparison(parsed);
  if (status !== 0) process.exitCode = status;
};

if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(scriptPath)) {
  try {
    main();
  } catch (error) {
    console.error(`${prefix} ${error.message}`);
    process.exitCode = 1;
  }
}
