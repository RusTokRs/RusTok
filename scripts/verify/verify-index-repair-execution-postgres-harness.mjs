#!/usr/bin/env node

import { readFile } from 'node:fs/promises';

const files = {
  support: 'crates/rustok-index/tests/support/drift_repair.rs',
  recovery: 'crates/rustok-index/tests/drift_repair_recovery_postgres_test.rs',
  execution: 'crates/rustok-index/tests/drift_repair_concrete_execution_postgres_test.rs',
  doc: 'crates/rustok-index/docs/m6-repair-execution-postgres-harness.md',
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

requireMarkers('support', [
  'RUSTOK_INDEX_TEST_DATABASE_URL',
  'DATABASE_URL',
  'CREATE SCHEMA',
  'CREATE TABLE tenants',
  'rustok_index::IndexModule.migrations()',
  'migration.up(&manager).await?',
  'migration.down(&manager).await?',
  'PostgresSchemaRegistrationStore',
  'PostgresMutationStore',
  'PostgresIndexDriftFindingWriter',
  'PostgresIndexDriftRepairStore',
  'PostgresIndexDriftRepairRecoveryStore',
  'RecoveryAwareIndexDriftRepairStore',
  'RecoveryAwareIndexDriftRepairOwner',
  'PostgresIndexDriftMissingEntityEvidenceReader',
  'PostgresIndexDriftMissingEntityRepairOwner',
  'PostgresIndexDriftOrphanLinkEvidenceReader',
  'PostgresIndexDriftOrphanLinkRepairOwner',
  'index_confirmed_missing_entity_evidence_v1',
  'index_confirmed_orphan_link_evidence_v1',
  'index_confirmed_orphan_link_identity_v1',
  'max_connections(1)',
  'SET search_path TO',
]);

requireMarkers('recovery', [
  'migrations_recovery_guard_and_concurrent_reservation_are_executable',
  'tokio::join!',
  'IndexDriftRepairNotStartedReason::FindingBusy',
  'count_repair_commands(&database, finding_id).await?, 1',
  'count_recovery_decisions(&database, winner_command_id)',
  'index_drift_repair_command_id_conflict',
  'IndexDriftRepairRecoveryAction::Pause',
  'IndexDriftRepairRecoveryOutcome::AlreadyApplied',
  'IndexDriftRepairRecoveryOutcome::StaleRevision',
  'force_complete_repair(&database, winner_command_id)',
  'IndexDriftRepairRecoveryAction::Resume',
  'mutate_completed_command',
  'database.migrate_down().await?',
  'index_consistency_finding_repair_recovery_decisions',
  'index_consistency_finding_repair_commands',
]);

requireMarkers('execution', [
  'missing_and_orphan_crash_windows_resume_exactly',
  'repair_evidence_after_commit_crash',
  'MISSING_DELIVERY_SOURCE',
  'ORPHAN_DELIVERY_SOURCE',
  'IndexDriftRepairOutcome::Repaired',
  'IndexDriftRepairOutcome::AlreadyCompleted',
  'exact_link_count(',
  'recovery_admission_fences_side_effect_and_completion',
  'GateBeforeOwnerEvidence',
  'GateAfterOwnerEvidence',
  'IndexDriftRepairRecoveryAction::Pause',
  'index_drift_repair_recovery_paused',
  'IndexDriftRepairRecoveryAction::Abandon',
  'index_drift_repair_recovery_abandoned',
  'orphan_commitments_and_normal_mutations_fail_closed',
  'source-moved',
  'link-substituted',
  'target-restored',
  'absence-moved',
  'normal-mutation',
  'before_not_repairable',
  'index_drift_repair_owner_unavailable',
]);

for (const name of ['support', 'recovery', 'execution']) {
  for (const forbidden of [
    '#[ignore]',
    'async_graphql',
    'Router::new',
    'ModuleRuntimeExtensions',
    'tokio::time::sleep',
    'repair_all',
    'automatic_repair',
  ]) {
    if (content[name].includes(forbidden)) {
      throw new Error(`${files[name]} contains forbidden harness capability ${forbidden}`);
    }
  }
}

requireMarkers('doc', [
  'Status: `source_ready_owner_execution_pending`.',
  'executable source, not admitted production evidence',
  '`RUSTOK_INDEX_TEST_DATABASE_URL`',
  'independent one-connection pools',
  '**pause before owner admission**',
  '**abandon after side effect but before completion**',
  'source-version movement',
  'exact link substitution',
  'authoritative target restoration',
  'target absence-version movement',
  'Tests, Node verifiers, formatting, Cargo checks',
]);

for (const forbidden of [
  'Status: `complete`',
  'PostgreSQL execution passed',
  'all scenarios passed',
  'CI passed',
]) {
  if (content.doc.includes(forbidden)) {
    throw new Error(`${files.doc} overclaims unexecuted evidence with ${forbidden}`);
  }
}

requireMarkers('plan', [
  'M6 - execute and admit concrete repair evidence',
  'source_ready_owner_execution_pending',
  'drift_repair_recovery_postgres_test',
  'drift_repair_concrete_execution_postgres_test',
]);
requireMarkers('aggregate', [
  "'verify-index-repair-execution-postgres-harness.mjs'",
]);

console.log('Index concrete repair PostgreSQL harness verified');
