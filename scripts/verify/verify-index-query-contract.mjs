#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const directory = path.dirname(fileURLToPath(import.meta.url));
const scripts = [
  'verify-index-query-planner.mjs',
  'verify-index-postgres-query-compiler.mjs',
  'verify-index-query-result-decoder.mjs',
  'verify-index-many-link-filtering.mjs',
  'verify-index-many-link-aggregate-ordering.mjs',
  'verify-index-decimal-aggregate-wire.mjs',
  'verify-index-query-snapshots.mjs',
  'verify-index-postgres-reference-equivalence.mjs',
  'verify-index-query-equivalence-capture.mjs',
  'verify-index-query-equivalence-admission.mjs',
  'verify-index-source-schema-registry.mjs',
  'verify-index-source-replay-contract.mjs',
  'verify-index-replay-job-leases.mjs',
  'verify-index-replay-multipage-runner.mjs',
  'verify-index-source-reconciliation.mjs',
  'verify-index-replay-runtime-composition.mjs',
  'verify-index-server-reconciliation-guard.mjs',
  'verify-index-reconciliation-retry-store.mjs',
  'verify-index-reconciliation-dead-letter-admission.mjs',
  'verify-index-reconciliation-dead-letter-inspection.mjs',
  'verify-index-reconciliation-dead-letter-requeue.mjs',
  'verify-index-replay-dry-run.mjs',
  'verify-index-replay-page-interruption.mjs',
  'verify-index-replay-retry-store.mjs',
  'verify-index-replay-dead-letter-admission.mjs',
  'verify-index-product-source.mjs',
  'verify-index-product-variant-source.mjs',
  'verify-index-product-graph-source.mjs',
  'verify-index-product-tombstone-source.mjs',
  'verify-index-sales-channel-source.mjs',
  'verify-index-query-runtime-composition.mjs',
  'verify-index-social-graph-privacy-consumer.mjs',
  'verify-social-graph-privacy-shadow-evidence.mjs',
];

for (const script of scripts) {
  const result = spawnSync(process.execPath, [path.join(directory, script)], {
    stdio: 'inherit',
    env: process.env,
  });
  if (result.error) {
    console.error(`[verify-index-query-contract] failed to start ${script}: ${result.error.message}`);
    process.exit(1);
  }
  if (result.signal) {
    console.error(`[verify-index-query-contract] ${script} terminated by ${result.signal}`);
    process.exit(1);
  }
  if (result.status !== 0) process.exit(result.status ?? 1);
}

console.log('[verify-index-query-contract] OK');
