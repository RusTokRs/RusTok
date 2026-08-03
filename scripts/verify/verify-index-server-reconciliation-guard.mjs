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
  'IndexReconciliationOperatorError::TenantMismatch',
  'permissions_for(&self.tenant_id, &self.actor_id)',
  'Permission::MODULES_MANAGE',
  'pub struct IndexReconciliationOperatorRuntime',
  'inner: rustok_index::PostgresIndexReconciliationRunner,',
  'dead_letters: rustok_index::infrastructure::postgres::PostgresIndexReconciliationDeadLetterInspector,',
  'Inspection(#[from] rustok_index::infrastructure::postgres::IndexReconciliationDeadLetterInspectionError),',
  'context.authorize_for(request.tenant_id())?;',
  'self.inner.run(request).await.map_err(Into::into)',
  'context.authorize_for(context.tenant_id())?;',
  '.request_cancel(context.tenant_id(), job_id)',
  'pub async fn inspect_dead_letter(',
  '.inspect(context.tenant_id(), job_id)',
  'extensions.get::<rustok_index::SharedIndexSourceRegistry>()',
  'extensions.get::<rustok_index::SharedIndexSchemaRegistry>()',
  'PostgresIndexReconciliationDeadLetterInspector::new(db.clone())',
  'PostgresIndexReconciliationRunner::new(db, sources, schemas.shared())',
  'missing_sources_do_not_publish_false_reconciliation_capability',
  'source_registry_without_shared_schema_registry_fails_closed',
  'complete_registries_publish_guarded_runtime_to_host_context',
  'duplicate_guarded_reconciliation_materialization_fails_closed',
  'operator_authorization_requires_exact_tenant_actor_and_modules_manage',
  'cross_tenant_run_is_denied_before_database_access',
  'dead_letter_inspection_authorizes_before_adapter_validation',
  'operator_context_rejects_nil_identity',
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
  fail(`${operatorPath} must reject tenant mismatch before request-bound permission evaluation`);
}

const runStart = operator.indexOf('    pub async fn run(');
const cancelStart = operator.indexOf('    pub async fn request_cancel(', runStart);
const inspectStart = operator.indexOf('    pub async fn inspect_dead_letter(', cancelStart);
const runtimeEnd = operator.indexOf('\n}\n\nimpl fmt::Debug', inspectStart);
if (
  runStart < 0
  || cancelStart <= runStart
  || inspectStart <= cancelStart
  || runtimeEnd <= inspectStart
) {
  fail(`${operatorPath} guarded run/cancel/inspection surface is malformed`);
}
const run = operator.slice(runStart, cancelStart);
const cancel = operator.slice(cancelStart, inspectStart);
const inspect = operator.slice(inspectStart, runtimeEnd);

if (
  run.indexOf('context.authorize_for(request.tenant_id())?;') < 0
  || run.indexOf('self.inner.run(request)')
    <= run.indexOf('context.authorize_for(request.tenant_id())?;')
) {
  fail(`${operatorPath} run must authorize the request tenant before runner delegation`);
}

for (const [name, body, delegation] of [
  ['cancellation', cancel, '.request_cancel(context.tenant_id(), job_id)'],
  ['inspection', inspect, '.inspect(context.tenant_id(), job_id)'],
]) {
  if (body.includes('tenant_id: Uuid')) {
    fail(`${operatorPath} ${name} must not accept a separate caller-supplied tenant`);
  }
  const auth = body.indexOf('context.authorize_for(context.tenant_id())?;');
  const delegate = body.indexOf(delegation);
  if (auth < 0 || delegate <= auth) {
    fail(`${operatorPath} ${name} must authorize and derive tenant scope from context`);
  }
}

for (const forbidden of [
  'last_error_details',
  'dependency_code: String',
  'DatabaseConnection',
  'PostgresIndexReconciliationDeadLetterInspector::new',
]) {
  if (inspect.includes(forbidden)) {
    fail(`${operatorPath} inspection method exposes forbidden adapter detail ${forbidden}`);
  }
}

const materializeStart = operator.indexOf(
  'pub(super) fn materialize_index_reconciliation_operator(',
);
const testsStart = operator.indexOf('\n#[cfg(test)]', materializeStart);
const materialize = operator.slice(materializeStart, testsStart);
const inspectorConstruction = materialize.indexOf(
  'PostgresIndexReconciliationDeadLetterInspector::new(db.clone())',
);
const runnerConstruction = materialize.indexOf(
  'PostgresIndexReconciliationRunner::new(db, sources, schemas.shared())',
);
const runtimeInsertion = materialize.indexOf(
  'extensions.insert(IndexReconciliationOperatorRuntime::new(',
);
if (
  inspectorConstruction < 0
  || runnerConstruction <= inspectorConstruction
  || runtimeInsertion <= runnerConstruction
) {
  fail(`${operatorPath} must compose inspector and runner into one guarded runtime`);
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
  'requeue',
  'retry_epoch',
  "state = 'pending'",
]) {
  if (productionOperator.includes(forbidden)) {
    fail(`${operatorPath} production boundary contains forbidden transport/SQL/recovery marker ${forbidden}`);
  }
}

requireMarkers(docsPath, [
  'Status: `source_complete_transport_and_recovery_work_pending`.',
  'requires effective `Permission::MODULES_MANAGE`',
  'retaining the pre-database denial boundary',
  'neither operation accepts a separate caller-selected tenant',
  'Inspection authorization runs before adapter validation or database access',
  'tenant-scoped read-only `inspect_dead_letter(context, job_id)`',
  'The existing server replay composition remains the single source-freezing point',
  'read-only dead-letter inspector over a clone of the host database handle',
  'authorized requeue with actor/reason audit',
  'canonical M6 drift-diagnosis and targeted-repair roadmap item remains open',
  'maintainer-run',
]);
requireMarkers(inspectionDocsPath, [
  'Status: `source_complete_transport_and_recovery_pending`.',
  '## Authorized server composition',
  'There is no caller-supplied tenant parameter.',
  'requires effective `modules:manage`',
  'GraphQL, HTTP, CLI, MCP, and admin transports remain open',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- [ ] Add drift diagnosis, targeted repair commands, and admitted repair evidence.',
  '- [ ] Add bounded retry/backoff, dead-letter state, and global scheduling ownership.',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-server-reconciliation-guard.mjs'",
  "'verify-index-reconciliation-dead-letter-inspection.mjs'",
  "'verify-index-replay-retry-store.mjs'",
  "'verify-index-replay-dead-letter-admission.mjs'",
]);

console.log('[verify-index-server-reconciliation-guard] OK');
