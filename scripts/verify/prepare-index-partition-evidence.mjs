#!/usr/bin/env node

import {
  existsSync,
  linkSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import path from 'node:path';
import { prepareManifest, renderShadowBootstrapSql } from './index-partition-evidence-core.mjs';

const prefix = '[prepare-index-partition-evidence]';

const parseArgs = (args) => {
  const values = new Map();
  for (let index = 0; index < args.length; index += 2) {
    const flag = args[index];
    const value = args[index + 1];
    if (!['--input', '--manifest', '--bootstrap'].includes(flag) || !value || value.startsWith('--')) {
      throw new Error('usage: --input <config.json> --manifest <manifest.json> --bootstrap <bootstrap.sql>');
    }
    if (values.has(flag)) throw new Error(`${flag} was provided more than once`);
    values.set(flag, value);
  }
  for (const flag of ['--input', '--manifest', '--bootstrap']) {
    if (!values.has(flag)) throw new Error(`${flag} is required`);
  }
  const options = {
    input: values.get('--input'),
    manifest: values.get('--manifest'),
    bootstrap: values.get('--bootstrap'),
  };
  const resolved = [options.input, options.manifest, options.bootstrap].map((value) => path.resolve(value));
  if (new Set(resolved).size !== resolved.length) {
    throw new Error('--input, --manifest, and --bootstrap must reference distinct paths');
  }
  return options;
};

const writeAtomicNew = (filename, content) => {
  if (existsSync(filename)) throw new Error(`refusing to overwrite existing file: ${filename}`);
  mkdirSync(path.dirname(filename), { recursive: true });
  const temporary = `${filename}.tmp-${process.pid}`;
  try {
    writeFileSync(temporary, content, { encoding: 'utf8', flag: 'wx' });
    linkSync(temporary, filename);
  } finally {
    rmSync(temporary, { force: true });
  }
};

try {
  const options = parseArgs(process.argv.slice(2));
  const config = JSON.parse(readFileSync(options.input, 'utf8'));
  const manifest = prepareManifest(config);
  const bootstrap = renderShadowBootstrapSql(manifest);
  writeAtomicNew(options.manifest, `${JSON.stringify(manifest, null, 2)}\n`);
  try {
    writeAtomicNew(options.bootstrap, bootstrap);
  } catch (error) {
    rmSync(options.manifest, { force: true });
    throw error;
  }
  console.log(`${prefix} prepared ${manifest.evidence_id}`);
} catch (error) {
  console.error(`${prefix} ${error.message}`);
  process.exitCode = 1;
}
