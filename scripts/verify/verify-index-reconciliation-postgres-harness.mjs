#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-reconciliation-postgres-harness] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const testPath =
  'crates/rustok-index/tests/source_reconciliation_postgres_test.rs';
const test = requireMarkers(testPath, [
  'const DATABASE_ENV: &str = "RUSTOK_INDEX_TEST_DATABASE_URL";',
  'std::env::var("DATABASE_URL")',
  'Uuid::new_v4().simple()',
  'CREATE SCHEMA',
  'SET search_path TO',
  '.max_connections(1)',
  'for migration in IndexModule.migrations()',
  'INSERT INTO index_schemas',
  'PostgresIndexReconciliationRunner::new(',
  'reconciliation_yield_resumes_across_new_connection_and_preserves_job_identity',
  'pending_reconciliation_cancel_is_durable_and_tenant_scoped',
  'IndexReconciliationRunStatus::Yielded',
  'IndexReconciliationRunStatus::Complete',
  'IndexReconciliationRunStatus::AlreadyComplete',
  'IndexReconciliationCancelOutcome::NotFound',
  'IndexReconciliationCancelOutcome::Cancelled',
  'IndexReconciliationTerminalState::Cancelled',
  'SELECT COUNT(*)::bigint AS value FROM index_entities',
  'SELECT state AS value FROM index_jobs WHERE kind = \'reconcile\'',
  "cursor->>'completed_passes'",
  "cursor->>'pages_processed'",
  'DROP SCHEMA IF EXISTS',
]);

for (const forbidden of [
  '#[ignore]',
  'tokio::spawn',
  'std::thread::sleep',
  'testcontainers',
  'TRUNCATE ',
  'DROP DATABASE',
  'CREATE DATABASE',
  'runtime_status: "passed"',
  'execution_status: "passed"',
]) {
  if (test.includes(forbidden)) {
    fail(`${testPath} contains forbidden marker ${forbidden}`);
  }
}

const connectionCalls = test.match(/fixture\.connection\(\)\.await\?/g) ?? [];
if (connectionCalls.length < 5) {
  fail(`${testPath} must reconstruct multiple scoped PostgreSQL connections`);
}

const testFunctions =
  test.match(/#\[tokio::test\]\s+async fn [a-z0-9_]+/g) ?? [];
if (testFunctions.length !== 2) {
  fail(`${testPath} must retain exactly two focused PostgreSQL cases`);
}

const docPath =
  'crates/rustok-index/docs/m6-reconciliation-postgres-harness.md';
const doc = requireMarkers(docPath, [
  'Status: `executable_no_run`',
  '`RUSTOK_INDEX_TEST_DATABASE_URL`',
  'restart-compatible connection reconstruction evidence',
  'reconciliation_yield_resumes_across_new_connection_and_preserves_job_identity',
  'pending_reconciliation_cancel_is_durable_and_tenant_scoped',
  'different tenant receiving `NotFound`',
  'terminal state `succeeded`',
  'durable job state `cancelled`',
  'does not claim a concurrent running-worker cancellation race',
  'The canonical M6 reconciliation and drift-repair item remains open',
  'The implementation agent intentionally did not run these commands',
  'node scripts/verify/verify-index-reconciliation-postgres-harness.mjs',
]);

for (const forbidden of [
  'Status: `complete`',
  'runtime passed',
  'PostgreSQL passed',
  'process restart proven',
  'database restart proven',
]) {
  if (doc.includes(forbidden)) {
    fail(`${docPath} contains an unsupported execution claim: ${forbidden}`);
  }
}

console.log('[verify-index-reconciliation-postgres-harness] OK');
