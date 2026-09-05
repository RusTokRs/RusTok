#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-linked-target-replay-redelivery-postgres-harness] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};
const forbidMarkers = (relative, source, markers) => {
  for (const marker of markers) {
    if (source.includes(marker)) fail(`${relative} contains forbidden marker ${marker}`);
  }
};

const harnessPath = 'crates/rustok-distribution/tests/product_linked_target_replay_redelivery_postgres.rs';
const harness = requireMarkers(harnessPath, [
  '#![cfg(feature = "mod-product")]',
  'replay_checkpoint_failure_duplicate_retry_and_late_stale_target_keep_graph_authoritative',
  'IndexReplayWorker::new(',
  'IndexReplayPageRequest::new(TENANT_ID, variant_schema_ref()?, 32)',
  'FailOnceCheckpointStore::new()',
  'IndexReplayFailure::retryable("injected_checkpoint_commit_failure")',
  'Err(IndexReplayError::CheckpointCommitFailed(_))',
  'assert!(checkpoint_store.checkpoint().is_none())',
  'let restarted_query = database.fresh_query_runtime().await?',
  'let restarted_worker = IndexReplayWorker::new(',
  'retry.status(), IndexReplayPageStatus::Complete',
  'retry.applied_count(), 0',
  'retry.duplicate_count(), 1',
  'retry.stale_count(), 0',
  'checkpoint.source_version(), Some(current_version)',
  'checkpoint.last_delivery_id()',
  'Some(current_delivery_id.as_str())',
  'HISTORICAL_SKU: &str = "variant-v2-never-applied"',
  'CURRENT_SKU: &str = "variant-v3-current"',
  '.apply_replay_mutation(',
  'IndexReplayMutationOutcome::StaleIgnored',
  'IndexReplayMutationOutcome::Duplicate',
  'materialized_variant_version(&database.mutation).await?',
  'current_version',
  'assert_scalar_product_visible(&runtime.query, true)',
  'assert_graph_visible(&runtime.query, false)',
  'assert_graph_payload(&runtime.query, CURRENT_SKU)',
  'assert_graph_payload(&restarted_query, CURRENT_SKU)',
  'materialize_postgres_index_sources',
  'materialize_index_source_registry',
  'materialize_postgres_index_query_runtime',
  'PostgresMutationStore::new',
  'PostgresSchemaRegistrationStore::new',
  'ModuleWorkRegistrations',
  'scheduler.run_once().await?',
  'rustok_channel::migrations::migrations()',
  'rustok_product::migrations::migrations()',
  'IndexModule.migrations()',
]);
forbidMarkers(harnessPath, harness, [
  'tokio::spawn',
  'loop {',
  'CREATE TABLE index_entities',
  'CREATE TABLE index_links',
  'INSERT INTO index_entities',
  'INSERT INTO index_links',
  'PostgresQueryEntityAdmission::new',
  'register_postgres_index_query_link_target_availability',
  'PostgresIndexReplayCheckpointStore::new',
]);

const replayWorkerPath = 'crates/rustok-index/src/application/source_replay.rs';
requireMarkers(replayWorkerPath, [
  'pub struct IndexReplayWorker',
  '.apply_replay_mutation(',
  '.commit_replay_checkpoint(&checkpoint)',
  'IndexReplayMutationOutcome::Applied => applied_count += 1',
  'IndexReplayMutationOutcome::Duplicate => duplicate_count += 1',
  'IndexReplayMutationOutcome::StaleIgnored => stale_count += 1',
  'CheckpointCommitFailed',
]);
const replaySinkPath = 'crates/rustok-index/src/infrastructure/postgres/source_replay.rs';
requireMarkers(replaySinkPath, [
  'impl IndexReplayMutationSink for PostgresMutationStore',
  'MutationDelivery::from_event(source_name, mutation.clone())',
  'MutationApplyOutcome::Applied { .. } => IndexReplayMutationOutcome::Applied',
  'MutationApplyOutcome::Duplicate { .. } => IndexReplayMutationOutcome::Duplicate',
  'MutationApplyOutcome::StaleIgnored { .. } =>',
  'IndexReplayMutationOutcome::StaleIgnored',
]);
const mutationStorePath = 'crates/rustok-index/src/infrastructure/postgres/mutation_store.rs';
requireMarkers(mutationStorePath, [
  '"applied" => Ok(MutationApplyOutcome::Duplicate {',
  'source_version: stored_source_version,',
  'if source_version <= current_source_version',
  'MutationApplyOutcome::StaleIgnored {',
  'incoming_source_version: source_version,',
  'current_source_version,',
  'self.lock_entity_key(transaction, mutation, backend).await?',
  'self.delete_existing_links(transaction, mutation, backend)',
]);
requireMarkers('crates/rustok-index/src/application/source_replay_tests.rs', [
  'checkpoint_failure_replays_the_same_event_delivery',
  'interruption_before_checkpoint_replays_applied_mutation_without_advancing_cursor',
  'vec![event_id, event_id]',
]);
requireMarkers('crates/rustok-index/docs/m7-linked-target-replay-redelivery-postgres-harness.md', [
  'Status: `source_ready_execution_pending`',
  'Crash after mutation durability, before checkpoint',
  'Worker restart and exact redelivery',
  'Late never-delivered historical mutation',
  'exact current replay after checkpoint loss -> inbox `Duplicate`',
  'previously unseen lower source version -> monotonic `StaleIgnored`',
  'fail-once checkpoint adapter',
  'source-ready and unexecuted',
]);

console.log('[verify-index-linked-target-replay-redelivery-postgres-harness] source-ready replay/redelivery composition packet verified');
