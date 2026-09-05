#!/usr/bin/env node

import { readFile } from 'node:fs/promises';

const files = {
  source: 'crates/rustok-index/src/infrastructure/postgres/drift_confirmed_candidate_writer.rs',
  postgresMod: 'crates/rustok-index/src/infrastructure/postgres/mod.rs',
  lib: 'crates/rustok-index/src/lib.rs',
  doc: 'crates/rustok-index/docs/m6-confirmed-candidate-finding-persistence.md',
  confirmationDoc: 'crates/rustok-index/docs/m6-drift-candidate-confirmation.md',
  lifecycleDoc: 'crates/rustok-index/docs/m6-drift-finding-lifecycle.md',
  repairDoc: 'crates/rustok-index/docs/m6-targeted-drift-repair.md',
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
  'mod drift_confirmed_candidate_writer;',
  'PostgresIndexDriftConfirmedCandidateWriter',
  'materialize_postgres_index_drift_confirmed_candidate_writer',
]);
requireMarkers('lib', [
  'PostgresIndexDriftConfirmedCandidateWriter',
  'IndexDriftConfirmedCandidateRecordOutcome',
  'materialize_postgres_index_drift_confirmed_candidate_writer',
]);
requireMarkers('source', [
  'pub struct PostgresIndexDriftConfirmedCandidateWriter',
  'pub async fn record_confirmed_candidate(',
  'Some(IsolationLevel::Serializable)',
  'Some(AccessMode::ReadWrite)',
  'materialized_candidate_matches(transaction, candidate).await?',
  'record_finding_in_transaction(transaction, request).await?',
  'IndexDriftConfirmedCandidateRecordOutcome::NotRecorded',
  'IndexDriftConfirmedCandidateNotRecordedReason::MaterializedChanged',
  'index.confirmed_missing_entity',
  'index.confirmed_orphan_link.',
  'index_confirmed_missing_entity_evidence_v1',
  'index_confirmed_orphan_link_evidence_v1',
  'index_confirmed_orphan_link_identity_v1',
  'target_absence_source_version().to_be_bytes()',
  'expected_digest == actual_digest',
  'SELECT pg_advisory_xact_lock(hashtextextended($1, 0))',
  'index-drift-finding\\u{1f}',
  'FOR SHARE',
  'ON CONFLICT (tenant_id, finding_key) DO NOTHING',
  'IndexDriftFindingWriteOutcome::Created',
  'IndexDriftFindingWriteOutcome::Refreshed',
  'IndexDriftFindingWriteOutcome::Reopened',
  'IndexDriftFindingWriteOutcome::Suppressed',
  'index_drift_digest_finding_v1',
  'pub fn is_retryable(&self) -> bool',
]);

const source = content.source.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'async_graphql',
  'Router::new',
  'ModuleRuntimeExtensions',
  'extensions.insert(',
  'tokio::spawn',
  'spawn_blocking',
  'IndexDriftRepairService',
  'IndexDriftRepairOwner',
  'repair_finding',
  'actor_id',
  'reason:',
  'SELECT *',
  ' OFFSET ',
]) {
  if (source.includes(forbidden)) {
    throw new Error(`confirmed candidate writer contains forbidden capability: ${forbidden}`);
  }
}

const recordStart = source.indexOf('    pub async fn record_confirmed_candidate(');
const begin = source.indexOf('.begin_with_config(', recordStart);
const inTx = source.indexOf('async fn record_in_transaction(');
const revalidate = source.indexOf('materialized_candidate_matches(transaction, candidate).await?', inTx);
const findingWrite = source.indexOf('record_finding_in_transaction(transaction, request).await?', revalidate);
const commit = source.indexOf('.commit()', begin);
if (
  recordStart < 0 ||
  begin <= recordStart ||
  commit <= begin ||
  inTx <= recordStart ||
  revalidate <= inTx ||
  findingWrite <= revalidate
) {
  throw new Error('confirmed candidate writer must begin, revalidate, persist, then commit');
}

const identity = source.indexOf('fn orphan_link_identity_digest(');
for (const marker of [
  'candidate.link_name().as_str().as_bytes()',
  'candidate.ordinal().to_be_bytes()',
  'hash_linked_key(&mut hasher, candidate.target())',
  'candidate.target_absence_source_version().to_be_bytes()',
]) {
  const position = source.indexOf(marker, identity);
  if (identity < 0 || position <= identity) {
    throw new Error(`orphan finding identity missing ${marker}`);
  }
}

requireMarkers('doc', [
  'Status: `source_complete_lifecycle_complete_targeted_repair_boundary_complete`.',
  '`SERIALIZABLE READ WRITE`',
  '`NotRecorded(MaterializedChanged)`',
  '`index.confirmed_orphan_link.<sha256>`',
  '`index_drift_digest_finding_v1`',
  'remains ignored and returns `Suppressed`',
  'm6-drift-finding-lifecycle.md',
  'm6-targeted-drift-repair.md',
  'No tests, verifiers, formatting, Cargo checks',
]);
requireMarkers('confirmationDoc', [
  'source_complete_persistence_complete_lifecycle_complete_targeted_repair_boundary_complete',
  'm6-confirmed-candidate-finding-persistence.md',
  'm6-targeted-drift-repair.md',
]);
requireMarkers('lifecycleDoc', [
  'Status: `source_complete_targeted_repair_boundary_complete`.',
  '`IndexDriftFindingAuthorizedLifecycleCommand`',
]);
requireMarkers('repairDoc', [
  'Status: `source_complete_recovery_aware_concrete_owners_execution_pending`.',
  'reproduces the persisted finding contract',
  '`PostgresMutationStore`',
]);
requireMarkers('plan', [
  'M6 - add prepared repair recovery policy',
  'source_complete_recovery_policy_pending',
]);
requireMarkers('aggregate', [
  "'verify-index-confirmed-candidate-persistence.mjs'",
  "'verify-index-targeted-drift-repair.mjs'",
  "'verify-index-missing-entity-repair-composition.mjs'",
]);

console.log('Index confirmed candidate persistence verified');
