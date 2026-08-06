#!/usr/bin/env node

import { readFile } from 'node:fs/promises';

const files = {
  app: 'crates/rustok-index/src/application/drift_repair.rs',
  appMod: 'crates/rustok-index/src/application/mod.rs',
  store: 'crates/rustok-index/src/infrastructure/postgres/drift_repair.rs',
  postgresMod: 'crates/rustok-index/src/infrastructure/postgres/mod.rs',
  migration:
    'crates/rustok-index/src/migrations/m20260806_000007_add_index_finding_repair_commands.rs',
  migrationsMod: 'crates/rustok-index/src/migrations/mod.rs',
  lib: 'crates/rustok-index/src/lib.rs',
  doc: 'crates/rustok-index/docs/m6-targeted-drift-repair.md',
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
  'mod drift_repair;',
  'IndexDriftAuthorizedRepairCommand',
  'IndexDriftRepairService',
  'IndexDriftRepairStore',
]);
requireMarkers('postgresMod', [
  'mod drift_repair;',
  'PostgresIndexDriftRepairStore',
  'materialize_postgres_index_drift_repair_store',
]);
requireMarkers('lib', [
  'PostgresIndexDriftRepairStore',
  'materialize_postgres_index_drift_repair_store',
]);
requireMarkers('migrationsMod', [
  'mod m20260806_000007_add_index_finding_repair_commands;',
  'Box::new(m20260806_000007_add_index_finding_repair_commands::Migration)',
  '"m20260806_000007_add_index_finding_repair_commands"',
  'vec!["m20260806_000006_add_index_finding_lifecycle_audit"]',
]);

requireMarkers('app', [
  'pub enum IndexDriftRepairTarget',
  'pub struct IndexDriftRepairCommand',
  'pub struct IndexDriftAuthorizedRepairCommand',
  'fn new(command: &IndexDriftRepairCommand) -> Self',
  'pub trait IndexDriftRepairAuthorizer: Send + Sync',
  'pub trait IndexDriftRepairEvidenceReader: Send + Sync',
  'pub trait IndexDriftRepairOwner: Send + Sync',
  'pub struct IndexDriftRepairOwnerRegistry',
  'pub trait IndexDriftRepairStore: Send + Sync',
  'pub struct IndexDriftRepairService',
  'self.authorizer.authorize(command).await?',
  'let authorized = IndexDriftAuthorizedRepairCommand::new(command);',
  'self.store.reserve(&authorized).await?',
  'self.evidence.capture_before(&authorized, &finding).await?',
  'owner.repair(&authorized, &finding, &before).await?',
  '.capture_after(&authorized, &finding, &before)',
  'self.store.complete(ticket, completion).await?',
  'IndexDriftRepairEvidenceState::Converged',
  'IndexDriftRepairOutcome::AlreadyCompleted',
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
  'repair_all',
  'while let',
  'loop {',
  'pub fn new(command: &IndexDriftRepairCommand) -> Self',
]) {
  if (productionApp.includes(forbidden)) {
    throw new Error(`targeted repair application contains forbidden capability: ${forbidden}`);
  }
}

const execute = productionApp.indexOf('    pub async fn execute(');
const authorize = productionApp.indexOf('self.authorizer.authorize(command).await?', execute);
const capability = productionApp.indexOf('IndexDriftAuthorizedRepairCommand::new(command)', authorize);
const reserve = productionApp.indexOf('self.store.reserve(&authorized).await?', capability);
const before = productionApp.indexOf('self.evidence.capture_before(&authorized, &finding).await?', reserve);
const owner = productionApp.indexOf('owner.repair(&authorized, &finding, &before).await?', before);
const after = productionApp.indexOf('.capture_after(&authorized, &finding, &before)', owner);
const complete = productionApp.indexOf('self.store.complete(ticket, completion).await?', after);
if (
  execute < 0 ||
  authorize <= execute ||
  capability <= authorize ||
  reserve <= capability ||
  before <= reserve ||
  owner <= before ||
  after <= owner ||
  complete <= after
) {
  throw new Error('repair service must authorize, reserve, capture before, call one owner, capture after, then complete');
}

requireMarkers('store', [
  'pub struct PostgresIndexDriftRepairStore',
  'impl IndexDriftRepairStore for PostgresIndexDriftRepairStore',
  'Some(IsolationLevel::Serializable)',
  'Some(AccessMode::ReadWrite)',
  'index-drift-repair-command\\u{1f}{tenant_id}\\u{1f}{command_id}',
  'SELECT pg_advisory_xact_lock(hashtextextended($1, 0))',
  "state = 'prepared'",
  'IndexDriftRepairNotStartedReason::FindingBusy',
  'details->>\'contract\' AS details_contract',
  'validate_target_commitment(',
  'index_confirmed_missing_entity_evidence_v1',
  'index_confirmed_orphan_link_evidence_v1',
  'index_confirmed_orphan_link_identity_v1',
  'index_drift_finding_key_v1',
  'index_drift_repair_command_v1',
  'UPDATE index_consistency_finding_repair_commands SET state = \'completed\'',
  'code: "finding_not_open".to_owned()',
  'IndexDriftRepairStoreCompletionOutcome::AlreadyCompleted',
]);

const productionStore = content.store.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'async_graphql',
  'Router::new',
  'ModuleRuntimeExtensions',
  'tokio::spawn',
  'spawn_blocking',
  'UPDATE index_consistency_findings SET',
  'DELETE FROM index_consistency_findings',
  'DELETE FROM index_consistency_finding_repair_commands',
  'SELECT *',
  ' OFFSET ',
  'tracing::',
]) {
  if (productionStore.includes(forbidden)) {
    throw new Error(`PostgreSQL targeted repair store contains forbidden capability: ${forbidden}`);
  }
}

for (const marker of [
  'hash_component(&mut hasher, MISSING_EVIDENCE_DOMAIN);',
  'hash_component(&mut hasher, state);',
  'hash_entity_key(&mut hasher, key);',
  'hash_component(&mut hasher, &indexed_source_version.to_be_bytes());',
  'hash_component(&mut hasher, &absence_source_version.to_be_bytes());',
]) {
  requireMarkers('store', [marker]);
}
for (const marker of [
  'hash_component(&mut hasher, ORPHAN_EVIDENCE_DOMAIN);',
  'hash_entity_key(&mut hasher, source_key);',
  'hash_component(&mut hasher, link_name.as_str().as_bytes());',
  'hash_component(&mut hasher, &ordinal.to_be_bytes());',
  'hash_linked_key(&mut hasher, target);',
  'hash_component(\n        &mut hasher,\n        &target_absence_source_version.to_be_bytes(),',
]) {
  requireMarkers('store', [marker]);
}

requireMarkers('migration', [
  'index_consistency_finding_repair_commands',
  'IndexFindingRepairCommands::CommandId',
  'IndexFindingRepairCommands::FindingId',
  'IndexFindingRepairCommands::PayloadDigest',
  'IndexFindingRepairCommands::BeforeDigest',
  'IndexFindingRepairCommands::AfterDigest',
  'IndexFindingRepairCommands::OwnerReceiptDigest',
  "CREATE UNIQUE INDEX {ACTIVE_INDEX} ON {TABLE_NAME} (tenant_id, finding_id) WHERE state = 'prepared'",
  "state IN ('prepared', 'completed')",
  "outcome IS NULL OR outcome IN ('repaired', 'not_repaired')",
  "OLD.state <> 'prepared' OR NEW.state <> 'completed'",
  'ensure_supported_backend(manager)?;',
]);
for (const forbidden of [
  'BEFORE DELETE ON',
  'DROP INDEX uq_index_finding_repair_active',
  'payload JSON',
]) {
  if (content.migration.includes(forbidden)) {
    throw new Error(`targeted repair migration contains forbidden marker: ${forbidden}`);
  }
}

requireMarkers('doc', [
  'Status: `source_complete_missing_entity_composed_recovery_pending`.',
  'cryptographic preimage check',
  '`SERIALIZABLE READ WRITE`',
  '`prepared -> completed`',
  '`materialize_postgres_index_drift_missing_entity_repair_service`',
  'mutation inbox',
  'prepared-reservation lease, expiry, abandonment, takeover, or operator recovery',
  'No tests, Node verifiers, formatting, Cargo checks',
]);
requireMarkers('concreteDoc', [
  'Status: `source_complete_recovery_policy_pending`.',
  'PostgresMutationStore::apply',
  'durable repair command UUID',
]);
requireMarkers('plan', [
  'M6 - add prepared repair recovery policy',
  'source_complete_recovery_policy_pending',
]);
requireMarkers('aggregate', [
  "'verify-index-targeted-drift-repair.mjs'",
  "'verify-index-missing-entity-repair-composition.mjs'",
]);

console.log('Index targeted drift repair boundary verified');
