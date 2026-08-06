#!/usr/bin/env node

import { readFile } from 'node:fs/promises';

const files = {
  source: 'crates/rustok-index/src/infrastructure/postgres/drift_orphan_link_repair.rs',
  postgresMod: 'crates/rustok-index/src/infrastructure/postgres/mod.rs',
  lib: 'crates/rustok-index/src/lib.rs',
  genericDoc: 'crates/rustok-index/docs/m6-targeted-drift-repair.md',
  doc: 'crates/rustok-index/docs/m6-orphan-link-repair-composition.md',
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
  'mod drift_orphan_link_repair;',
  'PostgresIndexDriftOrphanLinkEvidenceReader',
  'PostgresIndexDriftOrphanLinkRepairOwner',
  'materialize_postgres_index_drift_orphan_link_repair_service',
]);
requireMarkers('lib', [
  'PostgresIndexDriftOrphanLinkEvidenceReader',
  'PostgresIndexDriftOrphanLinkRepairOwner',
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
  'pub struct PostgresIndexOrphanLinkMutationStore',
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
  '.load_materialized(target, authorized.command().command_id())',
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
  'OrphanMaterializedLink::Absent,\n            OrphanMutationDeliveryState::Applied',
  'OrphanLinkEvidencePhase::After,\n            OrphanMaterializedLink::Absent,\n            OrphanMutationDeliveryState::Applied',
  'Some(IsolationLevel::Serializable)',
  'SELECT pg_advisory_xact_lock(hashtextextended($1, 0))',
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

const capture = production.indexOf('    async fn capture(');
const firstSource = production.indexOf('let first_source = self.load_source_authority(target).await?;', capture);
const firstTarget = production.indexOf('let first_target = self.load_target_authority(target).await?;', firstSource);
const materialized = production.indexOf('.load_materialized(target, authorized.command().command_id())', firstTarget);
const secondSource = production.indexOf('let second_source = self.load_source_authority(target).await?;', materialized);
const secondTarget = production.indexOf('let second_target = self.load_target_authority(target).await?;', secondSource);
const changed = production.indexOf('first_source != second_source || first_target != second_target', secondTarget);
if (
  capture < 0 ||
  firstSource <= capture ||
  firstTarget <= firstSource ||
  materialized <= firstTarget ||
  secondSource <= materialized ||
  secondTarget <= secondSource ||
  changed <= secondTarget
) {
  throw new Error('orphan evidence must bracket one materialized snapshot with stable source and target reads');
}

const ownerStart = production.indexOf('impl IndexDriftRepairOwner for PostgresIndexDriftOrphanLinkRepairOwner');
const ownerEnd = production.indexOf('\nimpl fmt::Debug for PostgresIndexDriftOrphanLinkRepairOwner', ownerStart);
if (ownerStart < 0 || ownerEnd <= ownerStart) {
  throw new Error('orphan-link repair owner implementation was not found');
}
const owner = production.slice(ownerStart, ownerEnd);
requireMarkers('source', [
  'self.mutations\n            .apply(authorized.command().command_id(), target)',
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
for (const marker of [
  'INSERT INTO index_inbox',
  'lock_entity_key(transaction, target.source_key).await?;',
  'require_exact_live_source(transaction, target).await?;',
  'require_exact_link(transaction, target).await?;',
  'DELETE FROM index_links',
  'complete_delivery(transaction, command_id, target, payload_digest).await?;',
]) {
  if (!mutationStore.includes(marker)) {
    throw new Error(`typed orphan-link mutation store missing ${marker}`);
  }
}

const classify = production.indexOf('fn classify_evidence(');
const beforeExact = production.indexOf(
  'OrphanLinkEvidencePhase::Before,\n            OrphanMaterializedLink::Exact,\n            OrphanMutationDeliveryState::Missing',
  classify,
);
const beforeRetry = production.indexOf(
  'OrphanLinkEvidencePhase::Before,\n            OrphanMaterializedLink::Absent,\n            OrphanMutationDeliveryState::Applied',
  classify,
);
const afterApplied = production.indexOf(
  'OrphanLinkEvidencePhase::After,\n            OrphanMaterializedLink::Absent,\n            OrphanMutationDeliveryState::Applied',
  classify,
);
if (classify < 0 || beforeExact <= classify || beforeRetry <= beforeExact || afterApplied <= beforeRetry) {
  throw new Error('orphan-link evidence phases must admit exact before, crash retry before, then applied after convergence');
}

requireMarkers('genericDoc', [
  'Status: `source_complete_recovery_aware_concrete_owners_execution_pending`.',
  '`materialize_postgres_index_drift_orphan_link_repair_service`',
  '`PostgresIndexOrphanLinkMutationStore`',
  'An absent link without the exact applied delivery is never accepted as convergence',
]);
requireMarkers('doc', [
  'Status: `source_complete_owner_execution_pending`.',
  '`index.confirmed_orphan_link.<sha256>`',
  '`index_drift_repair_orphan_link`',
  'delivery ID equal to the durable repair command UUID',
  'does not issue SQL',
  'Ordinals above the removed row are intentionally not rewritten',
  'No tests, Node verifiers, formatting, Cargo checks',
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
