#!/usr/bin/env node

import path from 'node:path';

import { buildRetainedPartitionArchiveManifest } from './index-partition-archive-manifest-core.mjs';
import { inspectRetainedPartitionBundle } from './index-partition-review-core.mjs';

const prefix = '[render-index-partition-archive-manifest]';

const usage = () => {
  console.log(`Usage:
  node scripts/verify/render-index-partition-archive-manifest.mjs \
    --root <retained-bundle-directory> \
    [--packet <partition-packet.json>] \
    [--admission <admission.json>]

The command validates one admitted retained bundle and prints a deterministic archive manifest to stdout. It writes no files.`);
};

const parseArgs = (args) => {
  if (args.length === 1 && ['--help', '-h'].includes(args[0])) return null;
  const values = new Map();
  for (let index = 0; index < args.length; index += 2) {
    const flag = args[index];
    const value = args[index + 1];
    if (!['--root', '--packet', '--admission'].includes(flag) || !value || value.startsWith('--')) {
      throw new Error('expected --root and optional --packet/--admission pairs');
    }
    if (values.has(flag)) throw new Error(`${flag} was provided more than once`);
    values.set(flag, value);
  }
  if (!values.has('--root')) throw new Error('--root is required');
  return {
    root: path.resolve(values.get('--root')),
    packetPath: values.has('--packet') ? path.resolve(values.get('--packet')) : undefined,
    admissionPath: values.has('--admission') ? path.resolve(values.get('--admission')) : undefined,
  };
};

try {
  const options = parseArgs(process.argv.slice(2));
  if (options === null) {
    usage();
  } else {
    const inspection = inspectRetainedPartitionBundle(options);
    const manifest = buildRetainedPartitionArchiveManifest(inspection);
    process.stdout.write(`${JSON.stringify(manifest, null, 2)}\n`);
  }
} catch (error) {
  console.error(`${prefix} ${error.message}`);
  process.exitCode = 1;
}
