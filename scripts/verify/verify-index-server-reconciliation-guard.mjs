#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-server-reconciliation-guard] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const compositionPath = 'apps/server/src/services/index_replay_runtime_composition.rs';
const operatorPath = 'apps/server/src/services/index_reconciliation_operator.rs';
const docsPath = 'apps/server/docs/index-reconciliation-operator-runtime.md';

const composition = requireMarkers(compositionPath, [
  '#[path = "index_reconciliation_operator.rs"]',
  'mod reconciliation_operator;',
  'materialize_postgres_index_replay_runtime(extensions, db.clone())',
  'reconciliation_operator::materialize_index_reconciliation_operator(extensions, db)?;',
]);
const replay = composition.indexOf('materialize_postgres_index_replay_runtime(extensions, db.clone())');
const reconciliation = composition.indexOf(
  'reconciliation_operator::materialize_index_reconciliation_operator(extensions, db)?;',
);
if (replay < 0 || reconciliation <= replay) {
  fail(`${compositionPath} must freeze replay/work composition before operator publication`);
}

const operator = requireMarkers(operatorPath, [
  'pub struct IndexReconciliationOperatorContext',
  'tenant_id.is_nil() || actor_id.is_nil()',
  'permissions_for(&self.tenant_id, &self.actor_id)',
  'Permission::MODULES_MANAGE',
  'pub struct IndexReconciliationOperatorRuntime',
  'inner: rustok_index::PostgresIndexReconciliationRunner,',
  'PostgresIndexReconciliationDeadLetterInspector,',
  'PostgresIndexDriftFindingInspector,',
  'PostgresIndexReconciliationRecoveryStore,',
  'DriftInspection(#[from] rustok_index::infrastructure::postgres::IndexDriftFindingInspectionError)',
  'context.authorize_for(request.tenant_id())?;',
  'pub async fn request_cancel(',
  'pub async fn inspect_dead_letter(',
  'pub async fn inspect_drift_finding(',
  'pub async fn requeue_dead_letter(',
  'IndexReconciliationRequeueRequest::new(',
  'context.tenant_id(),',
  'context.actor_id(),',
  '.requeue_failed(request)',
  'drift_finding_inspection_authorizes_before_adapter_validation',
]);
const production = operator.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'tokio::spawn', 'spawn_blocking', 'SELECT ', 'INSERT ', 'UPDATE ', 'DELETE ',
  'Router::new', 'async_graphql', 'ModuleWorkScheduler', 'StopHandle',
]) {
  if (production.includes(forbidden)) fail(`${operatorPath} contains ${forbidden}`);
}

const runStart = production.indexOf('    pub async fn run(');
const cancelStart = production.indexOf('    pub async fn request_cancel(', runStart);
const deadLetterStart = production.indexOf('    pub async fn inspect_dead_letter(', cancelStart);
const driftStart = production.indexOf('    pub async fn inspect_drift_finding(', deadLetterStart);
const requeueStart = production.indexOf('    pub async fn requeue_dead_letter(', driftStart);
const runtimeEnd = production.indexOf('\n}\n\nimpl fmt::Debug', requeueStart);
if ([runStart, cancelStart, deadLetterStart, driftStart, requeueStart, runtimeEnd].some((value) => value < 0)) {
  fail(`${operatorPath} guarded method segments are incomplete`);
}
const run = production.slice(runStart, cancelStart);
const cancel = production.slice(cancelStart, deadLetterStart);
const deadLetter = production.slice(deadLetterStart, driftStart);
const drift = production.slice(driftStart, requeueStart);
const requeue = production.slice(requeueStart, runtimeEnd);
if (run.indexOf('context.authorize_for(request.tenant_id())?;') < 0
    || run.indexOf('self.inner.run(request)') <= run.indexOf('context.authorize_for(request.tenant_id())?;')) {
  fail(`${operatorPath} run must authorize before delegation`);
}
for (const [name, body, marker] of [
  ['cancel', cancel, '.request_cancel(context.tenant_id(), job_id)'],
  ['dead-letter inspection', deadLetter, '.inspect(context.tenant_id(), job_id)'],
  ['drift-finding inspection', drift, '.inspect(context.tenant_id(), finding_id)'],
]) {
  const auth = body.indexOf('context.authorize_for(context.tenant_id())?;');
  const delegate = body.indexOf(marker);
  if (body.includes('tenant_id: Uuid') || auth < 0 || delegate <= auth) {
    fail(`${operatorPath} ${name} must bind tenant to authorized context`);
  }
}
const auth = requeue.indexOf('context.authorize_for(context.tenant_id())?;');
const request = requeue.indexOf('IndexReconciliationRequeueRequest::new(');
const tenant = requeue.indexOf('context.tenant_id(),', request);
const actor = requeue.indexOf('context.actor_id(),', tenant);
const delegate = requeue.indexOf('.requeue_failed(request)');
if (auth < 0 || request <= auth || tenant <= request || actor <= tenant || delegate <= actor) {
  fail(`${operatorPath} requeue must authorize, bind tenant/actor, then delegate`);
}

const driftComposition = production.indexOf('let drift_findings =');
const driftConstructor = production.indexOf('PostgresIndexDriftFindingInspector::new(db.clone())', driftComposition);
const runnerComposition = production.indexOf('PostgresIndexReconciliationRunner::new(db, sources, schemas.shared())');
const runtimeComposition = production.indexOf('IndexReconciliationOperatorRuntime::new(', runnerComposition);
const driftArgument = production.indexOf('drift_findings,', runtimeComposition);
if (driftComposition < 0 || driftConstructor <= driftComposition || runnerComposition <= driftConstructor
    || runtimeComposition <= runnerComposition || driftArgument <= runtimeComposition) {
  fail(`${operatorPath} must privately compose drift inspection before runtime publication`);
}

requireMarkers('crates/rustok-index/src/infrastructure/postgres/replay_runtime.rs', [
  'register_postgres_index_reconciliation_work(extensions)?;',
]);
requireMarkers('crates/rustok-index/src/infrastructure/postgres/source_reconciliation_scheduler.rs', [
  'impl ModuleWorkRegistration for IndexReconciliationWorkRegistration',
  'PostgresIndexReconciliationRunner',
]);
requireMarkers(docsPath, [
  'Status: `source_complete_transport_and_owner_execution_pending`.',
  'effective `Permission::MODULES_MANAGE`',
  'accept no caller-selected tenant',
  '`inspect_drift_finding(context, finding_id)`',
  'The same registry-freezing composition also publishes',
  'The operator runtime does not expose or own that scheduler',
  'existing generic server module-work bootstrap',
  'Drift inspection is read-only and is not scheduled.',
  'only authorizes bounded inspection of findings that already exist',
  'maintainer-run',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- [ ] Add drift diagnosis, targeted repair commands, and admitted repair evidence.',
  '- [ ] Add bounded retry/backoff, dead-letter state, and global scheduling ownership.',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-server-reconciliation-guard.mjs'",
  "'verify-index-reconciliation-host-scheduler.mjs'",
  "'verify-index-drift-finding-inspection.mjs'",
]);

console.log('[verify-index-server-reconciliation-guard] OK');
