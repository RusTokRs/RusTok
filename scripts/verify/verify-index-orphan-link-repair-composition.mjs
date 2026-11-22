#!/usr/bin/env node

import { readFile } from 'node:fs/promises';

const files = {
  source: 'crates/modules/rustok-index/src/infrastructure/postgres/drift_orphan_link_repair.rs',
  postgresMod: 'crates/modules/rustok-index/src/infrastructure/postgres/mod.rs',
  lib: 'crates/modules/rustok-index/src/lib.rs',
  genericDoc: 'crates/modules/rustok-index/docs/m6-targeted-drift-repair.md',
  doc: 'crates/modules/rustok-index/docs/m6-orphan-link-repair-composition.md',
  plan: 'crates/modules/rustok-index/docs/implementation-plan.md',
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
  'mod drift_orphan_link_repair;',
  'PostgresIndexDriftOrphanLinkEvidenceReader',
  'PostgresIndexDriftOrphanLinkRepairOwner',
  'PostgresIndexOrphanLinkAuthorityFence',
  'materialize_postgres_index_drift_orphan_link_repair_service',
]);
requireMarkers('lib', [
  'PostgresIndexDriftOrphanLinkEvidenceReader',
  'PostgresIndexDriftOrphanLinkRepairOwner',
  'PostgresIndexOrphanLinkAuthorityFence',
  'materialize_postgres_index_drift_orphan_link_repair_service',
]);
for (const forbidden of ['IndexOrphanLinkRemovalOutcome', 'PostgresIndexOrphanLinkMutationStore']) {
  if (content.postgresMod.includes(forbidden) || content.lib.includes(forbidden)) {
    throw new Error(`internal orphan-link mutation detail leaked through crate exports: ${forbidden}`);
  }
}

requireMarkers('source', [
  'pub struct PostgresIndexDriftOrphanLinkEvidenceReader',
  'impl IndexDriftRepairEvidenceReader for PostgresIndexDriftOrphanLinkEvidenceReader',
  'struct PostgresIndexOrphanLinkMutationStore',
  'pub trait PostgresIndexOrphanLinkAuthorityFence',
  'self.authority.verify_and_lock(&transaction, repair_target)',
  'pub struct PostgresIndexDriftOrphanLinkRepairOwner',
  'impl IndexDriftRepairOwner for PostgresIndexDriftOrphanLinkRepairOwner',
  'struct OrphanLinkOnlyRepairStore',
  'impl IndexDriftRepairStore for OrphanLinkOnlyRepairStore',
  'materialize_postgres_index_drift_orphan_link_repair_service(',
  'IndexDriftRepairTargetKind::OrphanLink',
  'IndexDriftRepairTarget::OrphanLink {',
  'IndexDriftRepairTarget::MissingEntity { .. } =>',
  'RecoveryAwareIndexDriftRepairOwner::new',
  'RecoveryAwareIndexDriftRepairStore::new',
  'index_drift_repair_orphan_link',
  'index_orphan_link_repair_evidence_v1',
  'index_orphan_link_removal_mutation_v1',
  'index_orphan_link_repair_owner_receipt_v1',
  'let first_source = self.load_source_authority(target).await?;',
  'let first_target = self.load_target_authority(target).await?;',
  '.load_materialized(target, command_id)',
  'let second_source = self.load_source_authority(target).await?;',
  'let second_target = self.load_target_authority(target).await?;',
  'first_source != second_source || first_target != second_target',
  'IndexSourceLoadRequest::new(vec![target.source_key.clone()])',
  'record_has_exact_link(',
  'target.target_absence_source_version',
  'Some(IsolationLevel::RepeatableRead)',
  'Some(AccessMode::ReadOnly)',
  'SELECT CAST(source_version AS TEXT) AS source_version_text, is_deleted FROM index_entities',
  'SELECT target_module, target_entity, target_schema_version, target_entity_id, target_locale_key FROM index_links',
  'SELECT mutation_kind, module_name, entity_name, schema_version, entity_id, locale_key, CAST(source_version AS TEXT) AS source_version_text, payload_hash, state FROM index_inbox',
  'OrphanLinkEvidencePhase::After,\n            OrphanMaterializedLink::Absent,\n            OrphanMutationDeliveryState::Applied',
  'Some(IsolationLevel::ReadCommitted)',
  'lock_entity_key(transaction, key, DbBackend::Postgres)',
  'INSERT INTO index_inbox',
  "'delete'",
  'command_id.to_string()',
  'link_removal_payload_digest(command_id, target)',
  'DELETE FROM index_links WHERE tenant_id = $1',
  'deleted.rows_affected() != 1',
  "UPDATE index_inbox SET state = 'applied'",
  'IndexOrphanLinkRemovalOutcome::Duplicate',
]);

const production = content.source.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'async_graphql',
  'Router::new',
  'ModuleRuntimeExtensions',
  'tokio::spawn',
  'spawn_blocking',
  'repair_all',
  'while let',
  'loop {',
  'UPDATE index_entities',
  'DELETE FROM index_entities',
  'UPDATE index_consistency_findings',
]) {
  if (production.includes(forbidden)) {
    throw new Error(`orphan-link repair composition contains forbidden capability: ${forbidden}`);
  }
}

function requireOrder(label, text, markers) {
  let cursor = 0;
  for (const marker of markers) {
    const position = text.indexOf(marker, cursor);
    if (position < 0) throw new Error(`${label} missing ordered marker ${marker}`);
    cursor = position + marker.length;
  }
}

const capture = production.slice(
  production.indexOf('    async fn capture('), production.indexOf('    async fn load_source_authority('),
);
requireOrder('orphan evidence', capture, [
  'if phase == OrphanLinkEvidencePhase::Before',
  'read_delivery_state(&self.db, command_id, target, MATERIALIZED_UNAVAILABLE).await?',
  '== OrphanMutationDeliveryState::Applied',
  'return recovery_evidence(command_id, target);',
  'let first_source = self.load_source_authority(target).await?;',
  'let first_target = self.load_target_authority(target).await?;',
  '.load_materialized(target, command_id)',
  'if phase == OrphanLinkEvidencePhase::Before',
  'materialized.delivery == OrphanMutationDeliveryState::Applied',
  'return recovery_evidence(command_id, target);',
  'let second_source = self.load_source_authority(target).await?;',
  'let second_target = self.load_target_authority(target).await?;',
  'first_source != second_source || first_target != second_target',
  'let state = classify_evidence(',
]);

const proofStart = production.indexOf('async fn read_delivery_state<');
const recoveryStart = production.indexOf('async fn recover_applied_delivery<', proofStart);
const recoveryEnd = production.indexOf('impl fmt::Debug for PostgresIndexOrphanLinkMutationStore', recoveryStart);
const proof = production.slice(proofStart, recoveryStart);
const readOnlyRecovery = production.slice(recoveryStart, recoveryEnd);
requireOrder('command-bound proof', proof, [
  'WHERE tenant_id = $1 AND source_name = $2 AND delivery_id = $3',
  'target.source_key.tenant_id.into()',
  'DELIVERY_SOURCE.to_owned().into()',
  'command_id.to_string().into()',
  'decode_delivery_state(',
  '&link_removal_payload_digest(command_id, target)',
]);
requireOrder('receipt-only recovery', readOnlyRecovery, [
  'match read_delivery_state(db, command_id, target, OWNER_UNAVAILABLE).await?',
  'OrphanMutationDeliveryState::Applied => Ok(IndexOrphanLinkRemovalOutcome::Duplicate)',
  'OrphanMutationDeliveryState::Missing => Err(retryable_failure(OWNER_UNAVAILABLE))',
]);
requireMarkers('source', [
  'stored_payload != payload_digest',
  '\"applied\" => Ok(OrphanMutationDeliveryState::Applied)',
  '\"resume_applied_delivery\"',
]);
for (const forbidden of ['apply(', 'apply_in_transaction(', 'INSERT ', 'UPDATE ', 'DELETE ', '.execute']) {
  if ((proof + readOnlyRecovery).includes(forbidden)) {
    throw new Error(`historical receipt recovery must not mutate indexes or recreate proof: ${forbidden}`);
  }
}

const ownerStart = production.indexOf('impl IndexDriftRepairOwner for PostgresIndexDriftOrphanLinkRepairOwner');
const ownerEnd = production.indexOf('\nimpl fmt::Debug for PostgresIndexDriftOrphanLinkRepairOwner', ownerStart);
if (ownerStart < 0 || ownerEnd <= ownerStart) {
  throw new Error('orphan-link repair owner implementation was not found');
}
const owner = production.slice(ownerStart, ownerEnd);
requireOrder('owner dispatch', owner, [
  'before.state() != IndexDriftRepairEvidenceState::Repairable',
  'if before.digest() == recovery_evidence_digest(command_id, target)',
  'recover_applied_delivery(&self.mutations.db, command_id, target).await?',
  '} else {',
  '.apply(command_id, authorized.command().target(), target)',
  'owner_receipt_digest(authorized, finding, target, outcome)',
]);
for (const forbidden of [
  'Statement::',
  'SELECT ',
  'INSERT ',
  'UPDATE ',
  'DELETE FROM',
  'index_links',
  'index_inbox',
]) {
  if (owner.includes(forbidden)) {
    throw new Error(`repair owner bypasses typed mutation store with ${forbidden}`);
  }
}

const mutationStart = production.indexOf('impl PostgresIndexOrphanLinkMutationStore');
const ownerStruct = production.indexOf('pub struct PostgresIndexDriftOrphanLinkRepairOwner', mutationStart);
if (mutationStart < 0 || ownerStruct <= mutationStart) {
  throw new Error('typed orphan-link mutation store must precede and own persistence for the repair owner');
}
const mutationStore = production.slice(mutationStart, ownerStruct);
requireOrder('transactional write authority', mutationStore, [
  '.begin_with_config(',
  'self.authority.verify_and_lock(&transaction, repair_target)',
  'self.apply_in_transaction(&transaction, command_id, target, &payload_digest)',
  '.commit()',
]);
requireOrder('fenced orphan-link mutation', mutationStore, [
  'async fn apply_in_transaction(',
  'INSERT INTO index_inbox',
  'if inserted.rows_affected() == 0',
  'return recover_applied_delivery(transaction, command_id, target).await;',
  'lock_entity_key(transaction, key, DbBackend::Postgres)',
  'require_exact_live_source(transaction, target).await?;',
  'if !target_absent_at(target_record, target.target_absence_source_version)',
  'require_exact_link(transaction, target).await?;',
  'DELETE FROM index_links',
  'complete_delivery(transaction, command_id, target, payload_digest).await?;',
]);

const classifyStart = production.indexOf('fn classify_evidence(');
const classifyEnd = production.indexOf('\nfn ', classifyStart + 1);
const classify = production.slice(classifyStart, classifyEnd);
requireOrder('live evidence classification', classify, [
  'OrphanLinkEvidencePhase::Before,\n            OrphanMaterializedLink::Exact,\n            OrphanMutationDeliveryState::Missing',
  'OrphanLinkEvidencePhase::After,\n            OrphanMaterializedLink::Absent,\n            OrphanMutationDeliveryState::Applied',
]);
if (classify.includes('OrphanLinkEvidencePhase::Before,\n            OrphanMaterializedLink::Absent')) {
  throw new Error('an absent edge can never admit a fresh destructive mutation');
}

requireMarkers('genericDoc', [
  'Status: `source_complete_recovery_aware_concrete_owners_execution_pending`.',
  '`materialize_postgres_index_drift_orphan_link_repair_service`',
  '`PostgresIndexOrphanLinkAuthorityFence`',
  'treats an absent link as convergence only when the exact command-bound inbox delivery',
]);
requireMarkers('doc', [
  'Status: `source_complete_owner_execution_pending`.',
  '`index.confirmed_orphan_link.<sha256>`',
  '`index_drift_repair_orphan_link`',
  'delivery ID equal to the durable repair command UUID',
  'does not issue SQL',
  'Ordinals above the removed row are intentionally not rewritten',
  'Double-reading authority is diagnostic, not a write fence.',
  'If the witness\nvanishes, recovery fails closed.',
  'NotRepaired(after_not_converged)',
]);
requireMarkers('plan', [
  'M6 - retain concrete repair execution evidence',
  'source_complete_recovery_aware_owner_execution_pending',
  'Compose one command-bound exact edge-removal persistence owner behind the recovery boundary.',
  'Require exact applied inbox proof before admitting an absent edge as convergence.',
]);
requireMarkers('aggregate', [
  "'verify-index-orphan-link-repair-composition.mjs'",
]);

console.log('Index orphan-link repair composition verified');
