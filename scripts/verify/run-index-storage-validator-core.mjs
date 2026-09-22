#!/usr/bin/env node

import {
  existsSync,
  linkSync,
  mkdtempSync,
  renameSync,
  rmSync,
} from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const reportFilenames = Object.freeze([
  'read-report.json',
  'mutation-report.json',
  'maintenance-report.json',
]);
const optionalResourceFilenames = Object.freeze([
  'runner-resources-before.txt',
  'runner-resources-after.txt',
]);

const forwardOutput = (stream, value) => {
  if (typeof value === 'string' && value.length !== 0) stream.write(value);
};

export const runValidatorCoreWithAtomicProvenance = ({
  evidenceRoot,
  corePath,
  environment = process.env,
  spawn = spawnSync,
  stdout = process.stdout,
  stderr = process.stderr,
}) => {
  const stagingRoot = mkdtempSync(path.join(evidenceRoot, '.provenance-validation-'));
  try {
    for (const filename of reportFilenames) {
      linkSync(path.join(evidenceRoot, filename), path.join(stagingRoot, filename));
    }
    for (const filename of optionalResourceFilenames) {
      const source = path.join(evidenceRoot, filename);
      if (existsSync(source)) linkSync(source, path.join(stagingRoot, filename));
    }

    const result = spawn(process.execPath, [corePath], {
      encoding: 'utf8',
      env: {
        ...environment,
        INDEX_BENCH_EVIDENCE_ROOT: stagingRoot,
      },
    });
    forwardOutput(stdout, result.stdout);
    forwardOutput(stderr, result.stderr);
    if (result.error) throw result.error;
    if (result.signal) {
      throw new Error(`validator core terminated by signal ${result.signal}`);
    }

    const status = result.status ?? 1;
    if (status !== 0) return status;

    const stagedProvenance = path.join(stagingRoot, 'provenance.json');
    if (!existsSync(stagedProvenance)) {
      throw new Error('validator core exited successfully without provenance.json');
    }
    renameSync(stagedProvenance, path.join(evidenceRoot, 'provenance.json'));
    return 0;
  } finally {
    rmSync(stagingRoot, { recursive: true, force: true });
  }
};
