#!/usr/bin/env node

import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { validatePartitionPacket } from './index-partition-evidence-core.mjs';

const prefix = '[validate-index-partition-evidence]';

const parseArgs = (args) => {
  const values = new Map();
  for (let index = 0; index < args.length; index += 2) {
    const flag = args[index];
    const value = args[index + 1];
    if (!['--input', '--output'].includes(flag) || !value || value.startsWith('--')) {
      throw new Error('usage: --input <packet.json> --output <admission.json>');
    }
    if (values.has(flag)) throw new Error(`${flag} was provided more than once`);
    values.set(flag, value);
  }
  for (const flag of ['--input', '--output']) {
    if (!values.has(flag)) throw new Error(`${flag} is required`);
  }
  const options = { input: values.get('--input'), output: values.get('--output') };
  if (path.resolve(options.input) === path.resolve(options.output)) {
    throw new Error('--input and --output must reference distinct paths');
  }
  return options;
};

const writeAtomic = (filename, content) => {
  mkdirSync(path.dirname(filename), { recursive: true });
  const temporary = `${filename}.tmp-${process.pid}`;
  try {
    writeFileSync(temporary, content, 'utf8');
    renameSync(temporary, filename);
  } finally {
    rmSync(temporary, { force: true });
  }
};

try {
  const options = parseArgs(process.argv.slice(2));
  if (!existsSync(options.input)) throw new Error(`missing packet: ${options.input}`);
  rmSync(options.output, { force: true });
  const packet = JSON.parse(readFileSync(options.input, 'utf8'));
  const admission = validatePartitionPacket(packet);
  writeAtomic(options.output, `${JSON.stringify(admission, null, 2)}\n`);
  console.log(`${prefix} ${admission.outcome} ${admission.evidence_id}`);
} catch (error) {
  console.error(`${prefix} ${error.message}`);
  process.exitCode = 1;
}
