#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-mutation-event-ack-contract] ${message}`);
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
  'mod mutation_event;',
  'IndexMutationEventCatalog',
  'IndexMutationEventWorker',
  'materialize_index_mutation_event_registry',
  'register_index_mutation_event',
]);

const contractPath = 'crates/rustok-index/src/application/mutation_event.rs';
const contract = requireMarkers(contractPath, [
  'pub struct IndexMutationEventDescriptor',
  'pub struct IndexMutationEventCatalog',
  'pub struct SharedIndexMutationEventRegistry',
  'pub struct IndexMutationEventDelivery<T>',
  'pub trait IndexMutationEventAcknowledger',
  'pub struct IndexMutationEventWorker<M, A>',
  'UnknownReplaySource',
  'ReplaySourceOwnerMismatch',
  'ReplaySourceSchemaMismatch',
  'MutationSchemaMismatch',
  'NilEventId',
  'ZeroSourceVersion',
  'NilTenantId',
  'NilEntityId',
  '.apply_replay_mutation(',
  '.acknowledge(&acknowledgement_token)',
  'mutation_failure_suppresses_acknowledgement',
  'acknowledgement_failure_is_reported_after_durable_apply',
  'unknown_domain_and_schema_mismatch_fail_before_apply_or_ack',
]);

const applyPosition = contract.indexOf('.apply_replay_mutation(');
const acknowledgePosition = contract.indexOf('.acknowledge(&acknowledgement_token)');
if (applyPosition < 0 || acknowledgePosition < 0 || applyPosition >= acknowledgePosition) {
  fail(`${contractPath} must apply the durable mutation before broker acknowledgement`);
}

for (const forbidden of [
  'iggy::',
  'rdkafka::',
  'async_nats::',
  'lapin::',
  'tokio::spawn',
  'std::thread::spawn',
  'sleep(',
  'acknowledgement_token.clone()',
  'format!("{acknowledgement_token',
  'tracing::',
]) {
  if (contract.includes(forbidden)) {
    fail(`${contractPath} contains forbidden broker/task/token coupling: ${forbidden}`);
  }
}

const sourceFactoryPath =
  'crates/rustok-index/src/infrastructure/postgres/source_factory.rs';
const sourceFactory = requireMarkers(sourceFactoryPath, [
  'materialize_index_mutation_event_registry(&staged)',
  'SharedIndexMutationEventRegistry',
  'MutationEventRegistry(#[source] IndexMutationEventError)',
  '*extensions = staged',
]);
if (
  sourceFactory.indexOf('materialize_index_mutation_event_registry(&staged)') >=
  sourceFactory.indexOf('*extensions = staged')
) {
  fail(`${sourceFactoryPath} must validate event routes before committing staged extensions`);
}

const docPath = 'crates/rustok-index/docs/m5-mutation-event-ack-contract.md';
requireMarkers(docPath, [
  'Status: `generic_contract_complete_social_graph_route_source_complete_runtime_execution_pending`',
  '`IndexMutationEventCatalog`',
  '`IndexMutationEventWorker`',
  'A mutation failure suppresses acknowledgement',
  'durable database commit followed by acknowledgement',
  'First production route: Social Graph',
  'Product, ProductVariant, or SalesChannel event routes',
  'The M5 implementation-plan item remains partially open',
  'Execution is maintainer-owned',
]);

requireMarkers('crates/rustok-index/docs/m5-social-graph-mutation-route.md', [
  'source_complete_runtime_execution_pending',
  'social_graph.relation.state_changed.v1',
  'Atomic source and route materialization',
  'existing Social Graph Iggy worker',
]);

console.log('[verify-index-mutation-event-ack-contract] OK');