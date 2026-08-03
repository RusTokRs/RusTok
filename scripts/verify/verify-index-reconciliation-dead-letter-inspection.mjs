#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-reconciliation-dead-letter-inspection] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const inspectorPath =
  'crates/rustok-index/src/infrastructure/postgres/source_reconciliation_dead_letter_inspector.rs';
const operatorPath = 'apps/server/src/services/index_reconciliation_operator.rs';
const schedulerPath =
  'crates/rustok-index/src/infrastructure/postgres/source_reconciliation_scheduler.rs';
const docsPath =
  'crates/rustok-index/docs/m6-reconciliation-dead-letter-inspection.md';
const planPath = 'crates/rustok-index/docs/implementation-plan.md';

const inspector = requireMarkers(inspectorPath, [
  'pub struct IndexReconciliationDeadLetterInspection',
  'pub struct PostgresIndexReconciliationDeadLetterInspector',
  'pub async fn inspect(',
  "kind = 'reconcile'",
  "state = 'failed'",
  'last_error_code',
  'last_error_details',
  '#[serde(deny_unknown_fields)]',
  'dependency_code',
  'retryable',
]);
const production = inspector.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'INSERT ', 'UPDATE ', 'DELETE ', 'tokio::spawn', 'tokio::time::sleep',
  'Router::new', 'async_graphql',
]) {
  if (production.includes(forbidden)) fail(`${inspectorPath} contains ${forbidden}`);
}

requireMarkers(operatorPath, [
  'pub async fn inspect_dead_letter(',
  'context.authorize_for(context.tenant_id())?;',
  '.inspect(context.tenant_id(), job_id)',
  'Permission::MODULES_MANAGE',
]);
requireMarkers(schedulerPath, [
  'IndexReconciliationRunStatus::FailedPermanent',
  'IndexReconciliationRunStatus::FailedExhausted',
]);
requireMarkers(docsPath, [
  'Status: `source_complete_transport_pending`.',
  'Raw diagnostic JSON is never returned',
  'there is no caller-supplied tenant parameter',
  'The module-owned host scheduler changes no inspector SQL',
  'Retryable jobs remain pending and are not inspectable',
  'maintainer-run',
]);
requireMarkers(planPath, [
  '- [ ] Add bounded retry/backoff, dead-letter state, and global scheduling ownership.',
  '- [ ] Add drift diagnosis, targeted repair commands, and admitted repair evidence.',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-reconciliation-dead-letter-inspection.mjs'",
  "'verify-index-reconciliation-host-scheduler.mjs'",
]);

console.log('[verify-index-reconciliation-dead-letter-inspection] OK');
