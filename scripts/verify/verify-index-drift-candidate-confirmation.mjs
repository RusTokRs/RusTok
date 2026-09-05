#!/usr/bin/env node

import { readFile } from 'node:fs/promises';

const files = {
  app: 'crates/rustok-index/src/application/drift_candidate_confirmation.rs',
  appMod: 'crates/rustok-index/src/application/mod.rs',
  observer: 'crates/rustok-index/src/infrastructure/postgres/drift_candidate_observer.rs',
  postgresMod: 'crates/rustok-index/src/infrastructure/postgres/mod.rs',
  lib: 'crates/rustok-index/src/lib.rs',
  doc: 'crates/rustok-index/docs/m6-drift-candidate-confirmation.md',
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

function countMarker(name, marker, expectedMinimum) {
  const count = content[name].split(marker).length - 1;
  if (count < expectedMinimum) {
    throw new Error(`${files[name]} expected at least ${expectedMinimum} occurrences of ${marker}`);
  }
}

requireMarkers('appMod', [
  'mod drift_candidate_confirmation;',
  'IndexDriftCandidateConfirmer',
  'IndexDriftConfirmedCandidate',
  'IndexDriftCandidateMaterializedObserver',
]);
requireMarkers('postgresMod', [
  'mod drift_candidate_observer;',
  'PostgresIndexDriftCandidateMaterializedObserver',
  'materialize_postgres_index_drift_candidate_confirmer',
]);
requireMarkers('lib', [
  'materialize_postgres_index_drift_candidate_confirmer',
  'PostgresIndexDriftCandidateMaterializedObserver',
]);
requireMarkers('app', [
  'pub trait IndexDriftCandidateMaterializedObserver: Send + Sync',
  'pub struct IndexDriftCandidateConfirmer',
  'pub async fn confirm_candidate(',
  'IndexDriftCandidateMaterializedObservation::Unchanged',
  'IndexDriftCandidateConfirmationOutcome::Confirmed',
  'IndexDriftCandidateConfirmationOutcome::NotCandidate',
  'IndexDriftConfirmedCandidate::MissingEntity',
  'IndexDriftConfirmedCandidate::OrphanLink',
  'first_absence < candidate.indexed_source_version()',
  'source_version == first_absence',
  'match &first_source',
  'source_version != candidate.indexed_source_version()',
  'record_has_exact_link(&record, candidate)?',
  'second_source != first_source',
  'source_version == first_target_absence',
  'absence.provider_for_schema(&key.schema).is_none()',
  'IndexSourceLoadRequest::new(vec![key.clone()])',
  'index_drift_candidate_confirmation_absence_unavailable',
  'index_drift_candidate_confirmation_source_changed',
]);
countMarker('app', 'self.materialized.observe_candidate(candidate).await?', 2);
countMarker('app', 'self.load_entity_authority(candidate.key()).await?', 2);
countMarker('app', 'self.load_source_link_authority(candidate).await?', 2);
countMarker('app', 'self.load_entity_authority(&target_key).await?', 2);

const productionApp = content.app.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'PostgresIndexDriftFindingWriter',
  'record_mismatch',
  'resolve_finding',
  'ignore_finding',
  'IndexDriftRepairService',
  'IndexDriftRepairOwner',
  'repair_finding',
  'tokio::spawn',
  'spawn_blocking',
  'async_graphql',
  'Router::new',
  'INSERT INTO',
  'UPDATE index_',
  'DELETE FROM',
]) {
  if (productionApp.includes(forbidden)) {
    throw new Error(`candidate confirmer contains forbidden capability: ${forbidden}`);
  }
}

const confirmStart = productionApp.indexOf('    pub async fn confirm_candidate(');
const firstObserve = productionApp.indexOf('self.materialized.observe_candidate(candidate).await?', confirmStart);
const dispatch = productionApp.indexOf('let outcome = match candidate', firstObserve);
const confirmedGate = productionApp.indexOf('if !outcome.is_confirmed()', dispatch);
const secondObserve = productionApp.indexOf('self.materialized.observe_candidate(candidate).await?', confirmedGate);
if (
  confirmStart < 0 ||
  firstObserve <= confirmStart ||
  dispatch <= firstObserve ||
  confirmedGate <= dispatch ||
  secondObserve <= confirmedGate
) {
  throw new Error('candidate confirmation must observe, confirm, gate, then observe again');
}

requireMarkers('observer', [
  'pub struct PostgresIndexDriftCandidateMaterializedObserver',
  'impl IndexDriftCandidateMaterializedObserver for PostgresIndexDriftCandidateMaterializedObserver',
  'SELECT CAST(source_version AS TEXT) AS source_version_text, is_deleted FROM index_entities',
  'source_version == candidate.indexed_source_version()',
  'SELECT EXISTS (SELECT 1 FROM index_entities s JOIN index_links l',
  'l.link_name = $8 AND l.ordinal = $9',
  'l.target_module = $10',
  '(t.tenant_id IS NULL OR (t.is_deleted = TRUE AND t.source_version > 0))',
  'IndexDriftCandidateMaterializedObservation::Unchanged',
  'IndexDriftCandidateMaterializedObservation::Changed',
  'materialize_postgres_index_drift_candidate_confirmer(',
  'extensions.get::<SharedIndexSourceRegistry>().cloned()',
  'extensions.get::<SharedIndexSourceAbsenceRegistry>().cloned()',
]);

const productionObserver = content.observer.split('\n#[cfg(test)]')[0];
for (const forbidden of [
  'INSERT INTO',
  'UPDATE index_',
  'DELETE FROM',
  'FOR UPDATE',
  'pg_advisory',
  'IndexDriftRepairService',
  'IndexDriftRepairOwner',
  'repair_finding',
  'tokio::spawn',
  'async_graphql',
  'Router::new',
  'extensions.insert(',
]) {
  if (productionObserver.includes(forbidden)) {
    throw new Error(`candidate materialized observer contains forbidden capability: ${forbidden}`);
  }
}

requireMarkers('doc', [
  'Status: `source_complete_persistence_complete_lifecycle_complete_targeted_repair_boundary_complete`.',
  'materialized identity',
  'An empty ordinary targeted load is never interpreted as absence.',
  'm6-confirmed-candidate-finding-persistence.md',
  'm6-drift-finding-lifecycle.md',
  'm6-targeted-drift-repair.md',
  'No tests, verifiers, formatting, Cargo checks',
]);
requireMarkers('lifecycleDoc', [
  'Status: `source_complete_targeted_repair_boundary_complete`.',
  'Fail-closed authorization',
]);
requireMarkers('repairDoc', [
  'Status: `source_complete_recovery_aware_concrete_owners_execution_pending`.',
  'The service does not scan findings',
  '`materialize_postgres_index_drift_missing_entity_repair_service`',
]);
requireMarkers('plan', [
  'M6 - add prepared repair recovery policy',
  'source_complete_recovery_policy_pending',
]);
requireMarkers('aggregate', [
  "'verify-index-drift-candidate-confirmation.mjs'",
  "'verify-index-targeted-drift-repair.mjs'",
  "'verify-index-missing-entity-repair-composition.mjs'",
]);

console.log('Index drift candidate confirmation boundary verified');
