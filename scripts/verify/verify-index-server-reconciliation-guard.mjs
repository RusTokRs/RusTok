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

const compositionPath =
  'apps/server/src/services/index_replay_runtime_composition.rs';
const operatorPath =
  'apps/server/src/services/index_reconciliation_operator.rs';
const docsPath =
  'apps/server/docs/index-reconciliation-operator-runtime.md';
const inspectionDocsPath =
  'crates/rustok-index/docs/m6-reconciliation-dead-letter-inspection.md';
const recoveryDocsPath =
  'crates/rustok-index/docs/m6-reconciliation-dead-letter-requeue.md';

const composition = requireMarkers(compositionPath, [
  '#[path = "index_reconciliation_operator.rs"]',
  'mod reconciliation_operator;',
  'pub use reconciliation_operator::{',
  'IndexReconciliationOperatorRuntime,',
  'materialize_postgres_index_replay_runtime(extensions, db.clone())',
  'reconciliation_operator::materialize_index_reconciliation_operator(extensions, db)?;',
]);
const replayMaterialization = composition.indexOf(
  'materialize_postgres_index_replay_runtime(extensions, db.clone())',
);
const reconciliationMaterialization = composition.indexOf(
  'reconciliation_operator::materialize_index_reconciliation_operator(extensions, db)?;',
);
if (
  replayMaterialization < 0
  || reconciliationMaterialization <= replayMaterialization
) {
  fail(`${compositionPath} must publish reconciliation only after replay source freezing`);
}

const operator = requireMarkers(operatorPath, [
  'pub struct IndexReconciliationOperatorContext',
  'tenant_id.is_nil() || actor_id.is_nil()',
  'permissions_for(&self.tenant_id, &self.actor_id)',
  'Permission::MODULES_MANAGE',
  'pub struct IndexReconciliationOperatorRuntime',
  'inner: rustok_index::PostgresIndexReconciliationRunner,',
  'dead_letters: rustok_index::infrastructure::postgres::PostgresIndexReconciliationDeadLetterInspector,',
  'recovery: rustok_index::infrastructure::postgres::PostgresIndexReconciliationRecoveryStore,',
  'Inspection(#[from] rustok_index::infrastructure::postgres::IndexReconciliationDeadLetterInspectionError),',
  'Recovery(#[from] rustok_index::infrastructure::postgres::IndexReconciliationRecoveryError),',
  'context.authorize_for(request.tenant_id())?;',
  'pub async fn request_cancel(',
  'pub async fn inspect_dead_letter(',
  'pub async fn requeue_dead_letter(',
  'IndexReconciliationRequeueRequest::new(',
  'context.tenant_id(),',
  'context.actor_id(),',
  '.requeue_failed(request)',
  'PostgresIndexReconciliationRecoveryStore::new(',
  'PostgresIndexReconciliationDeadLetterInspector::new(',
  'PostgresIndexReconciliationRunner::new(db, sources, schemas.shared())',
  'dead_letter_requeue_authorizes_before_request_validation',
]);

const authorizeStart = operator.indexOf('    fn authorize_for(');
const authorizeEnd = operator.indexOf('\n}\n\n#[derive(Debug, Error)]', authorizeStart);
if (authorizeStart < 0 || authorizeEnd <= authorizeStart) {
  fail(`${operatorPath} must retain one bounded authorize_for implementation`);
}
const authorize = operator.slice(authorizeStart, authorizeEnd);
const tenantCheck = authorize.indexOf('if requested_tenant != self.tenant_id');
const permissionLookup = authorize.indexOf(
  'permissions_for(&self.tenant_id, &self.actor_id)',
);
const manageCheck = authorize.indexOf('Permission::MODULES_MANAGE');
if (
  tenantCheck < 0
  || permissionLookup <= tenantCheck
  || manageCheck <= permissionLookup
) {
  fail(`${operatorPath} must reject tenant mismatch before permission evaluation`);
}

const runStart = operator.indexOf('    pub async fn run(');
const cancelStart = operator.indexOf('    pub async fn request_cancel(', runStart);
const inspectStart = operator.indexOf('    pub async fn inspect_dead_letter(', cancelStart);
const requeueStart = operator.indexOf('    pub async fn requeue_dead_letter(', inspectStart);
const runtimeEnd = operator.indexOf('\n}\n\nimpl fmt::Debug', requeueStart);
if (
  runStart < 0
  || cancelStart <= runStart
  || inspectStart <= cancelStart
  || requeueStart <= inspectStart
  || runtimeEnd <= requeueStart
) {
  fail(`${operatorPath} guarded surface is malformed`);
}
const run = operator.slice(runStart, cancelStart);
const cancel = operator.slice(cancelStart, inspectStart);
const inspect = operator.slice(inspectStart, requeueStart);
const requeue = operator.slice(requeueStart, runtimeEnd);

if (
  run.indexOf('context.authorize_for(request.tenant_id())?;') < 0
  || run.indexOf('self.inner.run(request)')
    <= run.indexOf('context.authorize_for(request.tenant_id())?;')
) {
  fail(`${operatorPath} run must authorize before runner delegation`);
}

for (const [name, body, delegation] of [
  ['cancellation', cancel, '.request_cancel(context.tenant_id(), job_id)'],
  ['inspection', inspect, '.inspect(context.tenant_id(), job_id)'],
]) {
  if (body.includes('tenant_id: Uuid')) {
    fail(`${operatorPath} ${name} must not accept a separate tenant`);
  }
  const auth = body.indexOf('context.authorize_for(context.tenant_id())?;');
  const delegate = body.indexOf(delegation);
  if (auth < 0 || delegate <= auth) {
    fail(`${operatorPath} ${name} must authorize and derive tenant from context`);
  }
}

for (const forbidden of ['tenant_id: Uuid', 'actor_id: Uuid']) {
  if (requeue.includes(forbidden)) {
    fail(`${operatorPath} recovery must not accept caller-selected ${forbidden}`);
  }
}
const recoveryAuth = requeue.indexOf(
  'context.authorize_for(context.tenant_id())?;',
);
const requestConstruction = requeue.indexOf(
  'IndexReconciliationRequeueRequest::new(',
);
const tenantBinding = requeue.indexOf('context.tenant_id(),', requestConstruction);
const actorBinding = requeue.indexOf('context.actor_id(),', tenantBinding);
const recoveryDelegation = requeue.indexOf('.requeue_failed(request)');
if (
  recoveryAuth < 0
  || requestConstruction <= recoveryAuth
  || tenantBinding <= requestConstruction
  || actorBinding <= tenantBinding
  || recoveryDelegation <= actorBinding
) {
  fail(`${operatorPath} recovery must authorize, bind context tenant/actor, then delegate`);
}
for (const forbidden of [
  'DatabaseConnection',
  'SELECT ',
  'INSERT ',
  'UPDATE ',
  'DELETE ',
  'last_error_details',
  "state = 'pending'",
]) {
  if (requeue.includes(forbidden)) {
    fail(`${operatorPath} recovery method contains forbidden engine detail ${forbidden}`);
  }
}

const materializeStart = operator.indexOf(
  'pub(super) fn materialize_index_reconciliation_operator(',
);
const testsStart = operator.indexOf('\n#[cfg(test)]', materializeStart);
const materialize = operator.slice(materializeStart, testsStart);
const recoveryConstruction = materialize.indexOf(
  'PostgresIndexReconciliationRecoveryStore::new(',
);
const inspectorConstruction = materialize.indexOf(
  'PostgresIndexReconciliationDeadLetterInspector::new(',
);
const runnerConstruction = materialize.indexOf(
  'PostgresIndexReconciliationRunner::new(db, sources, schemas.shared())',
);
const runtimeInsertion = materialize.indexOf(
  'extensions.insert(IndexReconciliationOperatorRuntime::new(',
);
if (
  recoveryConstruction < 0
  || inspectorConstruction <= recoveryConstruction
  || runnerConstruction <= inspectorConstruction
  || runtimeInsertion <= runnerConstruction
) {
  fail(`${operatorPath} must compose recovery, inspector, and runner into one runtime`);
}

const productionOperator = operator.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'tokio::spawn',
  'spawn_blocking',
  'SELECT ',
  'INSERT ',
  'UPDATE ',
  'DELETE ',
  'Router::new',
  'route(',
  'async_graphql',
  "state = 'pending'",
]) {
  if (productionOperator.includes(forbidden)) {
    fail(`${operatorPath} production boundary contains forbidden marker ${forbidden}`);
  }
}

requireMarkers(docsPath, [
  'Status: `source_complete_transport_and_scheduling_pending`.',
  'requires effective `Permission::MODULES_MANAGE`',
  'accept no caller-selected tenant',
  'accepts no caller-selected actor',
  'authorization run before adapter or recovery-request validation',
  'tenant/actor-bound `requeue_dead_letter(context, job_id, reason)`',
  'construct `IndexReconciliationRequeueRequest` from `context.tenant_id()`',
  'The same-job failed-to-pending reset',
  'canonical bounded retry/global scheduling and drift-diagnosis/targeted-repair roadmap items remain open',
  'maintainer-run',
]);
requireMarkers(inspectionDocsPath, [
  'Status: `source_complete_transport_pending`.',
  'beside the canonical reconciliation runner and audited recovery store',
  'There is no caller-supplied tenant parameter.',
  'same guarded runtime also exposes manual audited requeue',
  'GraphQL, HTTP, CLI, MCP, and admin transports remain open',
]);
requireMarkers(recoveryDocsPath, [
  'Status: `source_complete_authorized_server_composition_transport_pending`.',
  '## Authorized server composition',
  'accepts no tenant or actor argument',
  'requires effective `Permission::MODULES_MANAGE`',
  'Authorization occurs before job/reason validation',
  'GraphQL, HTTP, CLI, MCP, native admin',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- [ ] Add drift diagnosis, targeted repair commands, and admitted repair evidence.',
  '- [ ] Add bounded retry/backoff, dead-letter state, and global scheduling ownership.',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-server-reconciliation-guard.mjs'",
  "'verify-index-reconciliation-dead-letter-inspection.mjs'",
  "'verify-index-reconciliation-dead-letter-requeue.mjs'",
]);

console.log('[verify-index-server-reconciliation-guard] OK');
