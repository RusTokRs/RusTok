#!/usr/bin/env node

import { readFile } from 'node:fs/promises';

const files = {
  source:
    'crates/rustok-index/src/infrastructure/postgres/drift_missing_entity_repair.rs',
  postgresMod: 'crates/rustok-index/src/infrastructure/postgres/mod.rs',
  lib: 'crates/rustok-index/src/lib.rs',
  genericDoc: 'crates/rustok-index/docs/m6-targeted-drift-repair.md',
  doc: 'crates/rustok-index/docs/m6-missing-entity-repair-composition.md',
  recoveryDoc: 'crates/rustok-index/docs/m6-prepared-repair-recovery.md',
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

requireMarkers('postgresMod', [
  'mod drift_missing_entity_repair;',
  'PostgresIndexDriftMissingEntityEvidenceReader',
  'PostgresIndexDriftMissingEntityRepairOwner',
  'materialize_postgres_index_drift_missing_entity_repair_service',
  'RecoveryAwareIndexDriftRepairOwner',
  'RecoveryAwareIndexDriftRepairStore',
]);
requireMarkers('lib', [
  'PostgresIndexDriftMissingEntityEvidenceReader',
  'PostgresIndexDriftMissingEntityRepairOwner',
  'materialize_postgres_index_drift_missing_entity_repair_service',
]);

requireMarkers('source', [
  'pub struct PostgresIndexDriftMissingEntityEvidenceReader',
  'impl IndexDriftRepairEvidenceReader for PostgresIndexDriftMissingEntityEvidenceReader',
  'pub struct PostgresIndexDriftMissingEntityRepairOwner',
  'impl IndexDriftRepairOwner for PostgresIndexDriftMissingEntityRepairOwner',
  'struct MissingEntityOnlyRepairStore',
  'impl IndexDriftRepairStore for MissingEntityOnlyRepairStore',
  'materialize_postgres_index_drift_missing_entity_repair_service(',
  'RecoveryAwareIndexDriftRepairOwner::new(db.clone(), base_owner)',
  'RecoveryAwareIndexDriftRepairStore::new(db, store)',
  'IndexDriftRepairTargetKind::MissingEntity',
  'IndexDriftRepairTarget::MissingEntity',
  'IndexDriftRepairTarget::OrphanLink { .. } => Err(permanent_failure(TARGET_UNSUPPORTED))',
  'self.sources.load(request).await.map_err(map_source_error)?',
  '.load(key.clone())',
  'let first_authority = self.load_authority(target.key).await?;',
  'let materialized = self.load_materialized(target.key).await?;',
  'let second_authority = self.load_authority(target.key).await?;',
  'if first_authority != second_authority',
  'SELECT CAST(source_version AS TEXT) AS source_version_text, is_deleted FROM index_entities',
  'absence_version > indexed_version',
  'MissingEntityMaterialized::Deleted(indexed_version)',
  'indexed_version == target.absence_source_version',
  'IndexMutation::Delete {',
  'event_id: authorized.command().command_id()',
  'MutationDelivery::from_event(DELIVERY_SOURCE, mutation)',
  '.apply(self.schemas.as_ref(), &delivery)',
  'index_missing_entity_repair_evidence_v1',
  'index_missing_entity_repair_owner_receipt_v1',
  'index_drift_repair_missing_entity',
  'MutationApplyOutcome::Applied',
  'MutationApplyOutcome::Duplicate',
  'MutationApplyOutcome::StaleIgnored',
]);

const production = content.source.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'async_graphql',
  'Router::new',
  'ModuleRuntimeExtensions',
  'tokio::spawn',
  'spawn_blocking',
  'INSERT INTO',
  'UPDATE index_',
  'DELETE FROM',
  'SELECT *',
  ' OFFSET ',
  'index_links',
  'record.fields',
  'serde_json',
  'tracing::',
  'repair_all',
  'while let',
  'loop {',
]) {
  if (production.includes(forbidden)) {
    throw new Error(`missing-entity repair composition contains forbidden capability: ${forbidden}`);
  }
}

const capture = production.indexOf('    async fn capture(');
const first = production.indexOf('let first_authority = self.load_authority(target.key).await?;', capture);
const materialized = production.indexOf('let materialized = self.load_materialized(target.key).await?;', first);
const second = production.indexOf('let second_authority = self.load_authority(target.key).await?;', materialized);
const changed = production.indexOf('if first_authority != second_authority', second);
if (capture < 0 || first <= capture || materialized <= first || second <= materialized || changed <= second) {
  throw new Error('evidence capture must bracket the exact materialized read with stable owner reads');
}

const gate = production.indexOf('impl IndexDriftRepairStore for MissingEntityOnlyRepairStore');
const gateCheck = production.indexOf(
  'if authorized.command().target().kind() != IndexDriftRepairTargetKind::MissingEntity',
  gate,
);
const delegateReserve = production.indexOf('self.inner.reserve(authorized).await', gateCheck);
if (gate < 0 || gateCheck <= gate || delegateReserve <= gateCheck) {
  throw new Error('concrete repair store must reject unsupported targets before durable reservation');
}

const owner = production.indexOf('impl IndexDriftRepairOwner for PostgresIndexDriftMissingEntityRepairOwner');
const deleteMutation = production.indexOf('let mutation = IndexMutation::Delete {', owner);
const eventIdentity = production.indexOf('event_id: authorized.command().command_id()', deleteMutation);
const delivery = production.indexOf('MutationDelivery::from_event(DELIVERY_SOURCE, mutation)', eventIdentity);
const apply = production.indexOf('.apply(self.schemas.as_ref(), &delivery)', delivery);
const receipt = production.indexOf('owner_receipt_digest(authorized, finding, target, &outcome)', apply);
if (
  owner < 0 ||
  deleteMutation <= owner ||
  eventIdentity <= deleteMutation ||
  delivery <= eventIdentity ||
  apply <= delivery ||
  receipt <= apply
) {
  throw new Error('missing-entity owner must derive one command-bound delete, apply it, then digest the outcome');
}

requireMarkers('genericDoc', [
  'Status: `source_complete_recovery_aware_concrete_owners_execution_pending`.',
  '`materialize_postgres_index_drift_missing_entity_repair_service`',
  'rejects `OrphanLink` before the generic reservation store',
  '`PostgresMutationStore`',
  '`IndexDriftRepairRecoveryService`',
]);
requireMarkers('doc', [
  'Status: `source_complete_recovery_aware_owner_execution_pending`.',
  'strictly newer',
  '`MutationDelivery::from_event("index_drift_repair_missing_entity", mutation)`',
  'durable repair command UUID is therefore also the mutation event',
  'physically missing row',
  '`RecoveryAwareIndexDriftRepairOwner`',
  '`RecoveryAwareIndexDriftRepairStore`',
  'No tests, Node verifiers, formatting, Cargo checks',
]);
requireMarkers('recoveryDoc', [
  'Status: `source_complete_owner_execution_pending`.',
  'same command UUID',
]);
requireMarkers('plan', [
  'M6 - retain concrete repair execution evidence',
  'source_complete_recovery_aware_owner_execution_pending',
  'Bind missing-entity retry identity to the durable repair command UUID.',
  'Add fail-closed prepared-command pause/resume/abandon recovery and lifecycle coordination.',
]);
requireMarkers('aggregate', [
  "'verify-index-missing-entity-repair-composition.mjs'",
  "'verify-index-orphan-link-repair-composition.mjs'",
  "'verify-index-prepared-repair-recovery.mjs'",
]);

console.log('Index missing-entity repair composition verified');
