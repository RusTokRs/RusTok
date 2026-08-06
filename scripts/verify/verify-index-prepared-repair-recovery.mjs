#!/usr/bin/env node

import { readFile } from 'node:fs/promises';

const files = {
  app: 'crates/rustok-index/src/application/drift_repair_recovery.rs',
  appMod: 'crates/rustok-index/src/application/mod.rs',
  store: 'crates/rustok-index/src/infrastructure/postgres/drift_repair_recovery.rs',
  postgresMod: 'crates/rustok-index/src/infrastructure/postgres/mod.rs',
  composition:
    'crates/rustok-index/src/infrastructure/postgres/drift_missing_entity_repair.rs',
  migration:
    'crates/rustok-index/src/migrations/m20260806_000008_add_index_finding_repair_recovery.rs',
  migrationsMod: 'crates/rustok-index/src/migrations/mod.rs',
  doc: 'crates/rustok-index/docs/m6-prepared-repair-recovery.md',
  targetedDoc: 'crates/rustok-index/docs/m6-targeted-drift-repair.md',
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
  'mod drift_repair_recovery;',
  'IndexDriftAuthorizedRepairRecoveryCommand',
  'IndexDriftRepairRecoveryService',
  'IndexDriftRepairRecoveryStore',
]);
requireMarkers('postgresMod', [
  'mod drift_repair_recovery;',
  'PostgresIndexDriftRepairRecoveryStore',
  'RecoveryAwareIndexDriftRepairOwner',
  'RecoveryAwareIndexDriftRepairStore',
  'materialize_postgres_index_drift_repair_recovery_store',
]);
requireMarkers('migrationsMod', [
  'mod m20260806_000008_add_index_finding_repair_recovery;',
  'Box::new(m20260806_000008_add_index_finding_repair_recovery::Migration)',
  '"m20260806_000008_add_index_finding_repair_recovery"',
  'vec!["m20260806_000007_add_index_finding_repair_commands"]',
]);

requireMarkers('app', [
  'pub enum IndexDriftRepairRecoveryState',
  'Active,',
  'Paused,',
  'Abandoned,',
  'pub enum IndexDriftRepairRecoveryAction',
  'Resume,',
  'Pause,',
  'Abandon,',
  'pub struct IndexDriftRepairRecoveryCommand',
  'payload_digest: String',
  'decision_id: Uuid',
  'expected_revision: Option<u64>',
  'pub trait IndexDriftRepairRecoveryAuthorizer: Send + Sync',
  'pub trait IndexDriftRepairRecoveryStore: Send + Sync',
  'pub struct IndexDriftRepairRecoveryService',
  'self.authorizer.authorize(command).await?',
  'let authorized = IndexDriftAuthorizedRepairRecoveryCommand::new(command);',
  'self.store.apply(&authorized).await?',
  'IndexDriftRepairRecoveryOutcome::Denied',
  'IndexDriftRepairRecoveryStoreOutcome::StaleRevision',
  'IndexDriftRepairRecoveryStoreOutcome::InvalidTransition',
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
  'ModuleRuntimeExtensions',
  'tokio::spawn',
  'spawn_blocking',
  'SystemTime',
  'Instant',
  'chrono::',
  'while let',
  'loop {',
  'pub fn new(command: &IndexDriftRepairRecoveryCommand) -> Self',
]) {
  if (productionApp.includes(forbidden)) {
    throw new Error(`repair recovery application contains forbidden capability: ${forbidden}`);
  }
}

const execute = productionApp.indexOf('    pub async fn execute(');
const authorize = productionApp.indexOf('self.authorizer.authorize(command).await?', execute);
const capability = productionApp.indexOf(
  'IndexDriftAuthorizedRepairRecoveryCommand::new(command)',
  authorize,
);
const apply = productionApp.indexOf('self.store.apply(&authorized).await?', capability);
if (execute < 0 || authorize <= execute || capability <= authorize || apply <= capability) {
  throw new Error('repair recovery service must authorize before constructing capability and applying storage');
}

requireMarkers('store', [
  'pub struct PostgresIndexDriftRepairRecoveryStore',
  'impl IndexDriftRepairRecoveryStore for PostgresIndexDriftRepairRecoveryStore',
  'pub struct RecoveryAwareIndexDriftRepairStore',
  'impl IndexDriftRepairStore for RecoveryAwareIndexDriftRepairStore',
  'pub struct RecoveryAwareIndexDriftRepairOwner',
  'impl IndexDriftRepairOwner for RecoveryAwareIndexDriftRepairOwner',
  'Some(IsolationLevel::Serializable)',
  'Some(AccessMode::ReadWrite)',
  'index-drift-repair-command\\u{1f}{tenant_id}\\u{1f}{command_id}',
  'SELECT pg_advisory_xact_lock(hashtextextended($1, 0))',
  'index_drift_repair_recovery_required',
  'index_drift_repair_recovery_paused',
  'index_drift_repair_recovery_abandoned',
  'command_payload_digest(command)',
  'index_drift_repair_command_v1',
  'action: "activate"',
  'previous_state: "unclassified"',
  'new_state: "active"',
  'decision_id: command.command_id()',
  'require_active_repair_state(',
  'self.inner.repair(authorized, finding, before).await',
  'self.gate_ticket(ticket).await?;',
  'self.inner.complete(ticket, completion).await',
  'current_revision != command.expected_revision()',
  'existing.matches_operator_command(command)',
  'IndexDriftRepairRecoveryStoreOutcome::FindingNotOpen',
]);

const productionStore = content.store.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'async_graphql',
  'Router::new',
  'ModuleRuntimeExtensions',
  'tokio::spawn',
  'spawn_blocking',
  'SystemTime',
  'Instant',
  'chrono::',
  'UPDATE index_consistency_finding_repair_recovery_decisions',
  'DELETE FROM index_consistency_finding_repair_recovery_decisions',
  'UPDATE index_consistency_findings',
  'DELETE FROM index_consistency_findings',
  'SELECT *',
  ' OFFSET ',
  'tracing::',
]) {
  if (productionStore.includes(forbidden)) {
    throw new Error(`PostgreSQL repair recovery contains forbidden capability: ${forbidden}`);
  }
}

const ownerImpl = productionStore.indexOf(
  'impl IndexDriftRepairOwner for RecoveryAwareIndexDriftRepairOwner',
);
const ownerLock = productionStore.indexOf('lock_command_repair(&transaction', ownerImpl);
const ownerActive = productionStore.indexOf('require_active_repair_state(', ownerLock);
const ownerDelegate = productionStore.indexOf(
  'self.inner.repair(authorized, finding, before).await',
  ownerActive,
);
if (
  ownerImpl < 0 ||
  ownerLock <= ownerImpl ||
  ownerActive <= ownerLock ||
  ownerDelegate <= ownerActive
) {
  throw new Error('recovery-aware owner must hold the exact command fence and require active state before delegation');
}

for (const marker of [
  'hash_component(&mut hasher, COMMAND_PAYLOAD_DOMAIN);',
  'hash_component(&mut hasher, command.tenant_id().as_bytes());',
  'hash_component(&mut hasher, command.finding_id().as_bytes());',
  'hash_component(&mut hasher, command.command_id().as_bytes());',
  'hash_repair_target(&mut hasher, command.target());',
  'hash_component(&mut hasher, command.actor().kind().as_bytes());',
  'hash_component(&mut hasher, command.actor().subject().as_bytes());',
  'hash_component(&mut hasher, command.reason().as_bytes());',
]) {
  requireMarkers('store', [marker]);
}

requireMarkers('migration', [
  'index_consistency_finding_repair_recovery_decisions',
  'IndexFindingRepairRecoveryDecisions::TenantId',
  'IndexFindingRepairRecoveryDecisions::CommandId',
  'IndexFindingRepairRecoveryDecisions::DecisionId',
  'IndexFindingRepairRecoveryDecisions::Revision',
  'uq_index_finding_repair_recovery_revision',
  "action IN ('activate', 'resume', 'pause', 'abandon')",
  "previous_state IN ('unclassified', 'active', 'paused')",
  "new_state IN ('active', 'paused', 'abandoned')",
  "action = 'resume' AND previous_state IN ('unclassified', 'paused')",
  "action = 'pause' AND previous_state = 'active'",
  "action = 'abandon' AND previous_state IN ('unclassified', 'active', 'paused')",
  'Index finding repair recovery decisions are immutable',
  "recovery_state IS DISTINCT FROM 'active'",
  "COALESCE((SELECT new_state FROM {DECISION_TABLE}",
  'ensure_supported_backend(manager)?;',
]);

requireMarkers('composition', [
  'RecoveryAwareIndexDriftRepairOwner',
  'RecoveryAwareIndexDriftRepairStore',
  'RecoveryAwareIndexDriftRepairOwner::new(db.clone(), base_owner)',
  'RecoveryAwareIndexDriftRepairStore::new(db, store)',
]);

requireMarkers('doc', [
  'Status: `source_complete_owner_execution_pending`.',
  '`unclassified -> active`',
  '`active -> paused`',
  '`paused -> active`',
  '`index_drift_repair_recovery_required`',
  'does not infer the side-effect result',
  'No tests, Node verifiers, formatting, Cargo checks',
]);
requireMarkers('targetedDoc', [
  'Status: `source_complete_recovery_aware_orphan_pending`.',
  '`IndexDriftRepairRecoveryService`',
  'database trigger rejects `prepared -> completed`',
]);
requireMarkers('concreteDoc', [
  'Status: `source_complete_recovery_aware_owner_execution_pending`.',
  '`RecoveryAwareIndexDriftRepairOwner`',
  '`RecoveryAwareIndexDriftRepairStore`',
]);
requireMarkers('plan', [
  'M6 - compose concrete orphan-link repair',
  'M6 prepared repair pause/resume/abandon recovery policy:',
  'Add fail-closed prepared-command pause/resume/abandon recovery and lifecycle coordination.',
]);
requireMarkers('aggregate', [
  "'verify-index-prepared-repair-recovery.mjs'",
]);

console.log('Index prepared repair recovery verified');
