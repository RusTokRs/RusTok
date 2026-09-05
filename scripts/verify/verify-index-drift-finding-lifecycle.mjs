#!/usr/bin/env node

import { readFile } from 'node:fs/promises';

const files = {
  app: 'crates/rustok-index/src/application/drift_finding_lifecycle.rs',
  appMod: 'crates/rustok-index/src/application/mod.rs',
  store: 'crates/rustok-index/src/infrastructure/postgres/drift_finding_lifecycle.rs',
  postgresMod: 'crates/rustok-index/src/infrastructure/postgres/mod.rs',
  migration: 'crates/rustok-index/src/migrations/m20260806_000006_add_index_finding_lifecycle_audit.rs',
  migrationsMod: 'crates/rustok-index/src/migrations/mod.rs',
  lib: 'crates/rustok-index/src/lib.rs',
  doc: 'crates/rustok-index/docs/m6-drift-finding-lifecycle.md',
  repairDoc: 'crates/rustok-index/docs/m6-targeted-drift-repair.md',
  concreteDoc: 'crates/rustok-index/docs/m6-missing-entity-repair-composition.md',
  plan: 'crates/rustok-index/docs/implementation-plan-current-2026-08-03.md',
  aggregate: 'scripts/verify/verify-index-query-contract.mjs',
};

const content = Object.fromEntries(
  await Promise.all(
    Object.entries(files).map(async ([name, path]) => [name, await readFile(path, 'utf8')]),
  ),
);

function requireMarkers(name, markers) {
  for (const marker of markers) {
    if (!content[name].includes(marker)) {
      throw new Error(`${files[name]} missing ${marker}`);
    }
  }
}

requireMarkers('appMod', [
  'mod drift_finding_lifecycle;',
  'IndexDriftFindingAuthorizedLifecycleCommand',
  'IndexDriftFindingLifecycleService',
  'IndexDriftFindingLifecycleStore',
]);
requireMarkers('postgresMod', [
  'mod drift_finding_lifecycle;',
  'PostgresIndexDriftFindingLifecycleStore',
  'materialize_postgres_index_drift_finding_lifecycle_store',
]);
requireMarkers('lib', [
  'PostgresIndexDriftFindingLifecycleStore',
  'materialize_postgres_index_drift_finding_lifecycle_store',
]);
requireMarkers('migrationsMod', [
  'mod m20260806_000006_add_index_finding_lifecycle_audit;',
  'Box::new(m20260806_000006_add_index_finding_lifecycle_audit::Migration)',
  '"m20260806_000006_add_index_finding_lifecycle_audit"',
  'vec!["m20260804_000005_relax_index_finding_locale_scope"]',
]);

requireMarkers('app', [
  'const MAX_ACTOR_KIND_BYTES: usize = 32;',
  'const MAX_ACTOR_SUBJECT_BYTES: usize = 191;',
  'const MAX_REASON_BYTES: usize = 512;',
  'pub enum IndexDriftFindingLifecycleAction',
  'pub struct IndexDriftFindingLifecycleActor',
  'pub struct IndexDriftFindingLifecycleCommand',
  'expected_state != IndexDriftFindingState::Open',
  'pub struct IndexDriftFindingAuthorizedLifecycleCommand',
  'fn new(command: &IndexDriftFindingLifecycleCommand) -> Self',
  'pub trait IndexDriftFindingLifecycleAuthorizer: Send + Sync',
  'pub trait IndexDriftFindingLifecycleStore: Send + Sync',
  'async fn apply_authorized_lifecycle_command(',
  'authorized: &IndexDriftFindingAuthorizedLifecycleCommand',
  'IndexDriftFindingLifecycleOutcome::Denied',
  'let authorized = IndexDriftFindingAuthorizedLifecycleCommand::new(command);',
  '.apply_authorized_lifecycle_command(&authorized)',
]);

const productionApp = content.app.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'sea_orm',
  'DatabaseConnection',
  'SELECT ',
  'INSERT ',
  'UPDATE ',
  'DELETE FROM',
  'async_graphql',
  'Router::new',
  'tokio::spawn',
  'repair_finding',
  'allow_all',
  'AlwaysAllowed',
  'pub fn new(command: &IndexDriftFindingLifecycleCommand) -> Self',
  'apply_lifecycle_command(\n        &self,\n        command: &IndexDriftFindingLifecycleCommand',
]) {
  if (productionApp.includes(forbidden)) {
    throw new Error(`finding lifecycle application boundary contains forbidden capability: ${forbidden}`);
  }
}

const execute = productionApp.indexOf('    pub async fn execute(');
const authorize = productionApp.indexOf('self.authorizer.authorize(command).await?', execute);
const denied = productionApp.indexOf('IndexDriftFindingLifecycleOutcome::Denied', authorize);
const grant = productionApp.indexOf('IndexDriftFindingAuthorizedLifecycleCommand::new(command)', denied);
const storeCall = productionApp.indexOf('.apply_authorized_lifecycle_command(&authorized)', grant);
if (execute < 0 || authorize <= execute || denied <= authorize || grant <= denied || storeCall <= grant) {
  throw new Error('lifecycle service must authorize, deny fail-closed, mint capability, then call store');
}

requireMarkers('store', [
  'pub struct PostgresIndexDriftFindingLifecycleStore',
  'impl IndexDriftFindingLifecycleStore for PostgresIndexDriftFindingLifecycleStore',
  'async fn apply_authorized_lifecycle_command(',
  'let command = authorized.command();',
  'Some(IsolationLevel::Serializable)',
  'Some(AccessMode::ReadWrite)',
  'index-drift-finding-lifecycle\\u{1f}',
  'SELECT pg_advisory_xact_lock(hashtextextended($1, 0))',
  'SELECT finding_id, action, from_state, to_state, actor_kind, actor_subject, reason',
  'IndexDriftFindingLifecycleStoreOutcome::AlreadyApplied',
  'IndexDriftFindingLifecycleNotAppliedReason::FindingNotFound',
  'IndexDriftFindingLifecycleNotAppliedReason::StateChanged',
  'SELECT state FROM index_consistency_findings',
  'FOR UPDATE',
  'UPDATE index_consistency_findings SET state = $3, closed_at = CURRENT_TIMESTAMP',
  'INSERT INTO index_consistency_finding_lifecycle_events',
  'index_drift_finding_lifecycle_command_id_conflict',
  'index_drift_finding_lifecycle_storage_unavailable',
]);

const productionStore = content.store.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'async_graphql',
  'Router::new',
  'ModuleRuntimeExtensions',
  'tokio::spawn',
  'spawn_blocking',
  'repair_finding',
  'UPDATE index_consistency_finding_lifecycle_events',
  'DELETE FROM index_consistency_finding_lifecycle_events',
  'tracing::',
]) {
  if (productionStore.includes(forbidden)) {
    throw new Error(`PostgreSQL finding lifecycle store contains forbidden capability: ${forbidden}`);
  }
}

const applyMethod = productionStore.indexOf('    async fn apply_authorized_lifecycle_command(');
const begin = productionStore.indexOf('.begin_with_config(', applyMethod);
const applyCall = productionStore.indexOf('self.apply_in_transaction(&transaction, command).await', begin);
const commit = productionStore.indexOf('.commit()', applyCall);
if (applyMethod < 0 || begin <= applyMethod || applyCall <= begin || commit <= applyCall) {
  throw new Error('authorized lifecycle store must begin, apply atomically, then commit');
}

const applyTransaction = productionStore.indexOf('    async fn apply_in_transaction(');
const commandLock = productionStore.indexOf('lock_command_id(transaction, command).await?', applyTransaction);
const existing = productionStore.indexOf('load_existing_event(transaction, command).await?', commandLock);
const findingLock = productionStore.indexOf('lock_finding_state(transaction, command).await?', existing);
const update = productionStore.indexOf('UPDATE index_consistency_findings SET state = $3', findingLock);
const audit = productionStore.indexOf('INSERT INTO index_consistency_finding_lifecycle_events', update);
if (
  applyTransaction < 0 ||
  commandLock <= applyTransaction ||
  existing <= commandLock ||
  findingLock <= existing ||
  update <= findingLock ||
  audit <= update
) {
  throw new Error('lifecycle transaction must serialize command, replay-check, lock finding, update, then append audit');
}

requireMarkers('migration', [
  'index_consistency_finding_lifecycle_events',
  'IndexFindingLifecycleEvents::CommandId',
  'IndexFindingLifecycleEvents::FindingId',
  '(\n                                    IndexFindingLifecycleEvents::TenantId,\n                                    IndexFindingLifecycleEvents::FindingId,',
  '(\n                                    IndexConsistencyFindings::TenantId,\n                                    IndexConsistencyFindings::FindingId,',
  "action IN ('resolve', 'ignore')",
  "from_state = 'open'",
  "action = 'resolve' AND to_state = 'resolved'",
  "action = 'ignore' AND to_state = 'ignored'",
  'length(actor_kind) BETWEEN 1 AND 32',
  'length(actor_subject) BETWEEN 1 AND 191',
  'length(reason) BETWEEN 1 AND 512',
  'BEFORE UPDATE ON',
  'audit rows cannot be rewritten',
  'ensure_supported_backend(manager)?;',
]);
for (const forbidden of [
  'BEFORE UPDATE OR DELETE',
  'BEFORE DELETE ON',
  'SqlValue::Json',
]) {
  if (content.migration.includes(forbidden)) {
    throw new Error(`finding lifecycle migration contains forbidden marker: ${forbidden}`);
  }
}

requireMarkers('doc', [
  'Status: `source_complete_targeted_repair_boundary_complete`.',
  'Fail-closed authorization',
  '`IndexDriftFindingAuthorizedLifecycleCommand`',
  '`SERIALIZABLE READ WRITE`',
  '`AlreadyApplied`',
  'm6-targeted-drift-repair.md',
  'does not automatically resolve a finding',
  'No tests, verifiers, formatting, Cargo checks',
]);
requireMarkers('repairDoc', [
  'Status: `source_complete_recovery_aware_concrete_owners_execution_pending`.',
  'prepared -> completed',
  '`materialize_postgres_index_drift_missing_entity_repair_service`',
]);
requireMarkers('concreteDoc', [
  'Status: `source_complete_recovery_aware_owner_execution_pending`.',
  'does not support orphan-link repair',
]);
requireMarkers('plan', [
  'M6 - add prepared repair recovery policy',
  'M6 drift finding lifecycle commands: `source_complete`',
  'source_complete_recovery_policy_pending',
]);
requireMarkers('aggregate', [
  "'verify-index-drift-finding-lifecycle.mjs'",
  "'verify-index-targeted-drift-repair.mjs'",
  "'verify-index-missing-entity-repair-composition.mjs'",
]);

console.log('Index drift finding lifecycle boundary verified');
