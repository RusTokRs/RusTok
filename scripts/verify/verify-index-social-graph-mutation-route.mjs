#!/usr/bin/env node

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), 'utf8');

const source = read('crates/rustok-social-graph/src/index_source.rs');
const moduleSource = read('crates/rustok-social-graph/src/lib.rs');
const liveConsumer = read('crates/rustok-social-graph/src/index_consumer.rs');
const sourceFactory = read('crates/rustok-index/src/infrastructure/postgres/source_factory.rs');
const contract = read('crates/rustok-index/docs/m5-social-graph-mutation-route.md');
const ackContract = read('crates/rustok-index/docs/m5-mutation-event-ack-contract.md');

for (const marker of [
  'pub const SOCIAL_GRAPH_RELATION_INDEX_SOURCE: &str =',
  '"social_graph.relation.state_changed.v1"',
  'pub const SOCIAL_GRAPH_RELATION_INDEX_EVENT_DOMAIN: &str =',
  'pub const SOCIAL_GRAPH_RELATION_INDEX_SOURCE_FACTORY: &str =',
  '"social-graph-relation-index-source"',
  'impl PostgresIndexSourceFactory for SocialGraphRelationPostgresIndexSourceFactory',
  'impl IndexSource for SocialGraphRelationPostgresIndexSource',
  'register_postgres_index_source_factory(',
  'register_index_mutation_event(',
  'register_index_source(',
  'derive_index_source_event_id(',
  'SOCIAL_GRAPH_RELATION_REPLAY_EVENT_DOMAIN',
  '.filter(relation::Column::TenantId.eq(request.tenant_id()))',
  '.order_by_asc(relation::Column::Id)',
  '.limit(fetch_limit)',
  'request.limit() + 1',
  'IndexSourcePage::new(&request, mutations, next_cursor)',
  'IndexSourceLoadBatch::new(&request, mutations)',
  'social_graph_relation_index_mutation(',
]) {
  assert.ok(source.includes(marker), `Social Graph Index source is missing ${marker}`);
}

assert.ok(
  moduleSource.includes('pub mod index_source;') &&
    moduleSource.includes('register_social_graph_index_source_contracts(extensions)') &&
    moduleSource.includes('PostgresIndexSourceFactoryCatalog') &&
    moduleSource.includes('IndexMutationEventCatalog'),
  'SocialGraphModule must publish schema, replay factory, and mutation route together',
);

const liveSourceMatch = liveConsumer.match(
  /pub const SOCIAL_GRAPH_INDEX_SOURCE: &str = "([^"]+)";/,
);
const replaySourceMatch = source.match(
  /pub const SOCIAL_GRAPH_RELATION_INDEX_SOURCE: &str =\s*"([^"]+)";/,
);
assert.ok(liveSourceMatch, 'live Social Graph Index consumer source identity is missing');
assert.ok(replaySourceMatch, 'Social Graph replay source identity is missing');
assert.equal(
  replaySourceMatch[1],
  liveSourceMatch[1],
  'live and replay paths must share one Index inbox source identity',
);
for (const marker of [
  'social_graph_relation_index_mutation(',
  'MutationDelivery::from_event(SOCIAL_GRAPH_INDEX_SOURCE, mutation)',
  'pub async fn acknowledge_consumed(',
  '.acknowledge(consumed)',
  'self.acknowledge_consumed(&consumed).await?',
]) {
  assert.ok(
    liveConsumer.includes(marker),
    `live Social Graph consumer is missing canonical projection/ack marker ${marker}`,
  );
}

for (const marker of [
  'materialize_index_mutation_event_registry(&staged)',
  'SharedIndexMutationEventRegistry',
  'MutationEventRegistry(#[source] IndexMutationEventError)',
  'staged.insert(event_registry)',
  '*extensions = staged',
]) {
  assert.ok(
    sourceFactory.includes(marker),
    `PostgreSQL source composition is missing atomic event route marker ${marker}`,
  );
}

assert.ok(
  sourceFactory.indexOf('materialize_index_mutation_event_registry(&staged)') <
    sourceFactory.indexOf('*extensions = staged'),
  'event routes must validate before staged runtime extensions are committed',
);

for (const marker of [
  'source_complete_runtime_execution_pending',
  'social_graph.relation.state_changed.v1',
  'rustok-social-graph.relation-replay-v1',
  'limit + 1',
  'Atomic source and route materialization',
  'existing Social Graph Iggy worker',
]) {
  assert.ok(contract.includes(marker), `Social Graph mutation route contract is missing ${marker}`);
}

assert.ok(
  ackContract.includes('First production route: Social Graph') &&
    ackContract.includes('./m5-social-graph-mutation-route.md') &&
    ackContract.includes('Product, ProductVariant, or SalesChannel event routes'),
  'M5 acknowledgement contract must record the first route without claiming full owner coverage',
);

assert.ok(
  !source.includes('tokio::spawn') &&
    !source.includes('acknowledge(') &&
    !source.includes('index_entities') &&
    !source.includes('index_inbox'),
  'source registration must not start workers, acknowledge brokers, or write Index tables directly',
);

console.log(
  '[verify-index-social-graph-mutation-route] bounded Social Graph source and exact event route verified',
);