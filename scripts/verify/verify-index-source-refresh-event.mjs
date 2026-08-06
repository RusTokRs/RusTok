#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-source-refresh-event] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

requireMarkers('crates/rustok-index/src/application/mod.rs', [
  'mod source_refresh_event;',
  'mod source_refresh_event_tests;',
  'IndexSourceRefreshEventDelivery',
  'IndexSourceRefreshEventWorker',
  'IndexSourceRefreshEventProcessError',
]);

const contractPath = 'crates/rustok-index/src/application/source_refresh_event.rs';
const contract = requireMarkers(contractPath, [
  'pub struct IndexSourceRefreshEventDelivery<T>',
  'minimum_source_version: u64',
  'pub struct IndexSourceRefreshEventWorker<M, A>',
  'source_registry.source_for_schema(&key.schema)',
  'IndexSourceLoadRequest::new(vec![key.clone()])',
  '.load(request)',
  'source_version < minimum_source_version',
  'rebind_event_id(mutation, event_id)',
  '.apply_replay_mutation(',
  '.acknowledge(&acknowledgement_token)',
  'MissingSourceMutation',
  'AmbiguousSourceMutation',
  'SourceVersionBehind',
  'ReplaySourceMismatch',
]);

const routePosition = contract.indexOf('source_registry.source_for_schema(&key.schema)');
const loadPosition = contract.indexOf('.load(request)');
const fencePosition = contract.indexOf('source_version < minimum_source_version');
const applyPosition = contract.indexOf('.apply_replay_mutation(');
const acknowledgePosition = contract.indexOf('.acknowledge(&acknowledgement_token)');
if (
  routePosition < 0 ||
  loadPosition <= routePosition ||
  fencePosition <= loadPosition ||
  applyPosition <= fencePosition ||
  acknowledgePosition <= applyPosition
) {
  fail(`${contractPath} must route, load, fence, durably apply, and then acknowledge`);
}

for (const forbidden of [
  'iggy::',
  'rdkafka::',
  'async_nats::',
  'lapin::',
  'tokio::spawn',
  'std::thread::spawn',
  'sleep(',
  'SELECT ',
  'INSERT INTO',
  'UPDATE ',
  'DELETE FROM',
  'tracing::',
  'acknowledgement_token.clone()',
  'format!("{acknowledgement_token',
]) {
  if (contract.includes(forbidden)) {
    fail(`${contractPath} contains forbidden transport/task/SQL/token coupling: ${forbidden}`);
  }
}

requireMarkers('crates/rustok-index/src/application/source_refresh_event_tests.rs', [
  'canonical_source_mutation_is_rebound_committed_and_then_acknowledged',
  'missing_or_behind_source_state_suppresses_apply_and_ack',
  'schema_mismatch_fails_before_source_load_apply_or_ack',
  'delivery_rejects_invalid_identity_and_revision',
  'vec!["apply", "ack"]',
  'Some(Uuid::from_u128(7))',
]);

requireMarkers('crates/rustok-index/docs/m5-source-refresh-event.md', [
  'Status: `source_complete_owner_event_publication_and_runtime_wiring_pending`',
  '`IndexSourceRefreshEventWorker`',
  'one bounded `IndexSourceLoadRequest` for exactly one key',
  'replace the replay-only mutation UUID with the broker event UUID',
  'A missing result or a revision below the event fence',
  'does not add Product wire events or publish Product routes',
  'The primary implementation cursor remains `M6 - execute and admit concrete repair evidence`',
  'No tests, Node verifiers, Cargo checks',
]);

console.log('[verify-index-source-refresh-event] exact source refresh commit-before-ack contract verified');
