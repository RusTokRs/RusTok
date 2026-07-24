#!/usr/bin/env node

import path from 'node:path';
import { validatePacketReadOrdering } from './check-index-storage-read-ordering.mjs';

const prefix = '[validate-index-storage-evidence]';
const supportedScales = new Set(['smoke', '100k', '1m']);
const scale = process.env.INDEX_BENCH_SCALE;

const main = async () => {
  if (supportedScales.has(scale)) {
    const evidenceRoot = process.env.INDEX_BENCH_EVIDENCE_ROOT
      ?? path.join('evidence/index-storage', scale);
    validatePacketReadOrdering(evidenceRoot);
  }
  await import('./validate-index-storage-evidence-core.mjs');
};

try {
  await main();
} catch (error) {
  console.error(`${prefix} ${error.message}`);
  process.exitCode = 1;
}
