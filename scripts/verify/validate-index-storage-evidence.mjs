#!/usr/bin/env node

import { rmSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { validatePacketReadOrdering } from './check-index-storage-read-ordering.mjs';
import { runValidatorCoreWithAtomicProvenance } from './run-index-storage-validator-core.mjs';
import { validateRunnerResourceSnapshots } from './validate-index-storage-runner-resources.mjs';

const prefix = '[validate-index-storage-evidence]';
const supportedScales = new Set(['smoke', '100k', '1m']);
const scale = process.env.INDEX_BENCH_SCALE;
const corePath = fileURLToPath(new URL('./validate-index-storage-evidence-core.mjs', import.meta.url));

const invalidatePacketProvenance = (evidenceRoot) => {
  rmSync(path.join(evidenceRoot, 'provenance.json'), { force: true });
};

const main = () => {
  const args = process.argv.slice(2);
  if (args.length !== 0) {
    throw new Error(
      'validate-index-storage-evidence.mjs does not accept arguments; use INDEX_BENCH_SCALE and INDEX_BENCH_EVIDENCE_ROOT',
    );
  }
  if (!supportedScales.has(scale)) {
    throw new Error(`INDEX_BENCH_SCALE must be smoke, 100k, or 1m; got ${scale}`);
  }

  const evidenceRoot = process.env.INDEX_BENCH_EVIDENCE_ROOT
    ?? path.join('evidence/index-storage', scale);
  invalidatePacketProvenance(evidenceRoot);
  validatePacketReadOrdering(evidenceRoot);
  validateRunnerResourceSnapshots(evidenceRoot);
  const status = runValidatorCoreWithAtomicProvenance({ evidenceRoot, corePath });
  if (status !== 0) process.exitCode = status;
};

try {
  main();
} catch (error) {
  console.error(`${prefix} ${error.message}`);
  process.exitCode = 1;
}
