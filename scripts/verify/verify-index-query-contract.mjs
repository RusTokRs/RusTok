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
  'verify-index-schema-readiness.mjs',
  'verify-index-source-replay-contract.mjs',
  'verify-index-source-refresh-event.mjs',
  'verify-index-source-absence-watermark.mjs',
  'verify-index-source-continuation.mjs',
  'verify-index-source-continuation-server.mjs',
  'verify-index-product-absence-postgres-harness.mjs',
  'verify-index-replay-job-leases.mjs',
  'verify-index-replay-multipage-runner.mjs',
  'verify-index-source-reconciliation.mjs',
  'verify-index-replay-runtime-composition.mjs',
  'verify-index-server-reconciliation-guard.mjs',
  'verify-index-drift-diagnosis-graphql-transport.mjs',
  'verify-index-drift-source-page-diagnosis.mjs',
  'verify-index-drift-source-page-graphql-transport.mjs',
  'verify-index-drift-candidate-contract.mjs',
  'verify-index-postgres-drift-candidate-reader.mjs',
  'verify-index-drift-candidate-confirmation.mjs',
  'verify-index-confirmed-candidate-persistence.mjs',
  'verify-index-drift-finding-lifecycle.mjs',
  'verify-index-targeted-drift-repair.mjs',
  'verify-index-missing-entity-repair-composition.mjs',
  'verify-index-orphan-link-repair-composition.mjs',
  'verify-index-prepared-repair-recovery.mjs',
  'verify-index-repair-execution-postgres-harness.mjs',
  'verify-index-repair-retained-evidence.mjs',
  'verify-index-reconciliation-retry-store.mjs',
  'verify-index-reconciliation-runner-retry.mjs',
  'verify-index-reconciliation-host-scheduler.mjs',
  'verify-index-drift-finding-inspection.mjs',
  'verify-index-drift-finding-writer.mjs',
  'verify-index-drift-digest-producer.mjs',
  'verify-index-drift-snapshot-reader.mjs',
  'verify-index-drift-finding-locale-scope.mjs',
  'verify-index-drift-finding-postgres-harness.mjs',
  'verify-index-reconciliation-dead-letter-admission.mjs',
  'verify-index-reconciliation-dead-letter-inspection.mjs',
  'verify-index-reconciliation-dead-letter-requeue.mjs',
  'verify-index-replay-dry-run.mjs',
  'verify-index-replay-page-interruption.mjs',
  'verify-index-replay-retry-store.mjs',
  'verify-index-replay-dead-letter-admission.mjs',
  'verify-index-product-source.mjs',
  'verify-index-product-locale-refresh-ledger.mjs',
  'verify-index-product-variant-refresh-ledger.mjs',
  'verify-index-product-refresh-canonical-writer.mjs',
  'verify-index-product-refresh-relay-step.mjs',
  'verify-index-product-channel-relation-ledger.mjs',
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
