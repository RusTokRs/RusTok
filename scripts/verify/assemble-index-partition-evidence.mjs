#!/usr/bin/env node

import {
  existsSync,
  linkSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import path from 'node:path';

import { assemblePartitionPacket } from './index-partition-evidence-assembly-core.mjs';

const prefix = '[assemble-index-partition-evidence]';
const identityOf = (stat) => `${stat.dev}:${stat.ino}`;

const parseArgs = (args) => {
  const values = new Map();
  for (let index = 0; index < args.length; index += 2) {
    const flag = args[index];
    const value = args[index + 1];
    if (!['--manifest', '--capture', '--output'].includes(flag)
        || !value || value.startsWith('--')) {
      throw new Error('usage: --manifest <manifest.json> --capture <capture.json> --output <packet.json>');
    }
    if (values.has(flag)) throw new Error(`${flag} was provided more than once`);
    values.set(flag, value);
  }
  for (const flag of ['--manifest', '--capture', '--output']) {
    if (!values.has(flag)) throw new Error(`${flag} is required`);
  }
  const options = {
    manifest: path.resolve(values.get('--manifest')),
    capture: path.resolve(values.get('--capture')),
    output: path.resolve(values.get('--output')),
  };
  if (new Set(Object.values(options)).size !== 3) {
    throw new Error('--manifest, --capture, and --output must reference distinct paths');
  }
  return options;
};

const readRegularJson = (filename, label) => {
  const stat = lstatSync(filename);
  if (stat.isSymbolicLink() || !stat.isFile()) {
    throw new Error(`${label} must be a regular non-symlink file`);
  }
  try {
    return {
      value: JSON.parse(readFileSync(filename, 'utf8')),
      identity: identityOf(stat),
    };
  } catch (error) {
    throw new Error(`${label} must contain valid UTF-8 JSON: ${error.message}`);
  }
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
  const manifestFile = readRegularJson(options.manifest, 'manifest');
  const captureFile = readRegularJson(options.capture, 'capture');
  if (manifestFile.identity === captureFile.identity) {
    throw new Error('--manifest and --capture must not alias the same file');
  }
  const { packet, resolvedPaths, identities } = assemblePartitionPacket({
    manifest: manifestFile.value,
    capturePath: options.capture,
    capture: captureFile.value,
  });
  if ([...resolvedPaths.values()].includes(options.output)) {
    throw new Error('--output must not alias a raw artifact path');
  }
  const retainedInputIdentities = new Set([
    manifestFile.identity,
    captureFile.identity,
    ...identities.values(),
  ]);
  if (retainedInputIdentities.size !== identities.size + 2) {
    throw new Error('manifest, capture, and raw artifacts must be distinct files');
  }
  writeAtomicNew(options.output, `${JSON.stringify(packet, null, 2)}\n`);
  console.log(`${prefix} assembled ${packet.manifest.evidence_id}`);
} catch (error) {
  console.error(`${prefix} ${error.message}`);
  process.exitCode = 1;
}
