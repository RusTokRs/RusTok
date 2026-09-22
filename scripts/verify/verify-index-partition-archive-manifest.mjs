#!/usr/bin/env node

import path from 'node:path';

import { verifySavedRetainedPartitionArchiveManifest } from './index-partition-archive-manifest-core.mjs';
import {
  publishDerivedJsonOutsideRetainedBundle,
  renderDerivedJson,
} from './index-partition-derived-output-core.mjs';
import { inspectRetainedPartitionBundle } from './index-partition-review-core.mjs';

const prefix = '[verify-index-partition-archive-manifest]';

const usage = () => {
  console.log(`Usage:
  node scripts/verify/verify-index-partition-archive-manifest.mjs \
    --root <retained-bundle-directory> \
    --manifest <saved-archive-manifest.json> \
    [--packet <partition-packet.json>] \
    [--admission <admission.json>] \
    [--output <verification-receipt.json>]

The saved manifest must be a regular non-symlink file outside the retained bundle. Stdout mode: The command writes no files. With --output, it atomically creates one new regular receipt outside the retained bundle and refuses overwrite.`);
};

const parseArgs = (args) => {
  if (args.length === 1 && ['--help', '-h'].includes(args[0])) return null;
  const values = new Map();
  for (let index = 0; index < args.length; index += 2) {
    const flag = args[index];
    const value = args[index + 1];
    if (!['--root', '--manifest', '--packet', '--admission', '--output'].includes(flag)
        || !value || value.startsWith('--')) {
      throw new Error('expected --root/--manifest and optional --packet/--admission/--output pairs');
    }
    if (values.has(flag)) throw new Error(`${flag} was provided more than once`);
    values.set(flag, value);
  }
  if (!values.has('--root')) throw new Error('--root is required');
  if (!values.has('--manifest')) throw new Error('--manifest is required');
  return {
    root: path.resolve(values.get('--root')),
    manifestPath: path.resolve(values.get('--manifest')),
    packetPath: values.has('--packet') ? path.resolve(values.get('--packet')) : undefined,
    admissionPath: values.has('--admission') ? path.resolve(values.get('--admission')) : undefined,
    outputPath: values.has('--output') ? path.resolve(values.get('--output')) : undefined,
  };
};

try {
  const options = parseArgs(process.argv.slice(2));
  if (options === null) {
    usage();
  } else {
    const inspection = inspectRetainedPartitionBundle(options);
    const receipt = verifySavedRetainedPartitionArchiveManifest({
      inspection,
      root: options.root,
      manifestPath: options.manifestPath,
    });
    if (options.outputPath === undefined) {
      process.stdout.write(renderDerivedJson(receipt));
    } else {
      publishDerivedJsonOutsideRetainedBundle({
        root: options.root,
        outputPath: options.outputPath,
        value: receipt,
        label: 'archive verification receipt output',
      });
    }
  }
} catch (error) {
  console.error(`${prefix} ${error.message}`);
  process.exitCode = 1;
}
