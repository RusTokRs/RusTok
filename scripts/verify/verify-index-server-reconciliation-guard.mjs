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
  'context.authorize_for(request.tenant_id())?;',
  'self.inner.run(request).await.map_err(Into::into)',
  'context.authorize_for(context.tenant_id())?;',
  '.request_cancel(context.tenant_id(), job_id)',
  'extensions.get::<rustok_index::SharedIndexSourceRegistry>()',
  'extensions.get::<rustok_index::SharedIndexSchemaRegistry>()',
  'PostgresIndexReconciliationRunner::new(db, sources, schemas.shared())',
  'missing_sources_do_not_publish_false_reconciliation_capability',
  'source_registry_without_shared_schema_registry_fails_closed',
  'complete_registries_publish_guarded_runtime_to_host_context',
  'duplicate_guarded_reconciliation_materialization_fails_closed',
  'operator_authorization_requires_exact_tenant_actor_and_modules_manage',
  'cross_tenant_run_is_denied_before_database_access',
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
const runtimeEnd = operator.indexOf('\n}\n\nimpl fmt::Debug', cancelStart);
if (runStart < 0 || cancelStart <= runStart || runtimeEnd <= cancelStart) {
  fail(`${operatorPath} guarded run/cancel surface is malformed`);
}
const run = operator.slice(runStart, cancelStart);
const cancel = operator.slice(cancelStart, runtimeEnd);
if (
  run.indexOf('context.authorize_for(request.tenant_id())?;') < 0
  || run.indexOf('self.inner.run(request)')
    <= run.indexOf('context.authorize_for(request.tenant_id())?;')
) {
  fail(`${operatorPath} run must authorize the request tenant before runner delegation`);
}
if (cancel.includes('tenant_id: Uuid')) {
  fail(`${operatorPath} cancellation must not accept a separate caller-supplied tenant`);
}
if (
  cancel.indexOf('context.authorize_for(context.tenant_id())?;') < 0
  || cancel.indexOf('.request_cancel(context.tenant_id(), job_id)')
    <= cancel.indexOf('context.authorize_for(context.tenant_id())?;')
) {
  fail(`${operatorPath} cancellation must authorize and derive tenant scope from context`);
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
  'PostgresIndexReconciliationDeadLetterInspector',
  'inspect_dead_letter',
  'requeue',
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
  'there is no separate caller-selected tenant parameter',
  'single source-freezing point',
  'publishes no false reconciliation capability',
  'reconciliation failed-scope admission',
  'bounded reconciliation dead-letter inspection',
  'authorized requeue with actor/reason audit',
  'canonical M6 drift-diagnosis and targeted-repair roadmap item remains open',
  'maintainer-run',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- [ ] Add drift diagnosis, targeted repair commands, and admitted repair evidence.',
  '- [ ] Add bounded retry/backoff, dead-letter state, and global scheduling ownership.',
]);
requireMarkers('scripts/verify/verify-index-query-contract.mjs', [
  "'verify-index-server-reconciliation-guard.mjs'",
  "'verify-index-replay-retry-store.mjs'",
  "'verify-index-replay-dead-letter-admission.mjs'",
]);

console.log('[verify-index-server-reconciliation-guard] OK');
