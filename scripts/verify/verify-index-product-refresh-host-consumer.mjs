#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-refresh-host-consumer] ${message}`);
  process.exit(1);
};
const requireMarker = (source, marker, label) => {
  if (!source.includes(marker)) fail(`${label} is missing ${marker}`);
};

const bridge = read('crates/rustok-distribution/src/product_index/refresh_event.rs');
const host = read('apps/server/src/services/product_index_refresh_worker.rs');
const bootstrap = read('apps/server/src/services/server_bootstrap.rs');
const genericWorker = read('crates/rustok-index/src/application/source_refresh_event.rs');
const workflow = read('.github/workflows/index-contract-ci.yml');

for (const marker of [
  'pub enum ProductIndexRefreshDelivery<T>',
  'pub struct ProductIndexRefreshDeliveryWorker<M, A>',
  'pub fn locale(',
  'pub fn variant(',
  'pub async fn process(',
]) {
  requireMarker(bridge, marker, 'distribution Product refresh host bridge');
}

for (const marker of [
  'RUSTOK_PRODUCT_INDEX_REFRESH_CONSUMER_ENABLED',
  'rustok-product-index-refresh',
  'PRODUCT_INDEX_REFRESH_TOPIC: &str = "domain"',
  'PersistentContractConsumerGroup',
  'PersistentContractDelivery::Event',
  'PersistentContractDelivery::DecodeFailure',
  'type Token = ConsumedContractEvent;',
  'ProductIndexRefreshEvent::LocaleRefreshRequested',
  'ProductIndexRefreshEvent::VariantRefreshRequested',
  'ProductIndexRefreshDelivery::locale(',
  'ProductIndexRefreshDelivery::variant(',
  'ProductIndexRefreshDeliveryWorker::new(',
  'PostgresMutationStore::new(',
  'SharedIndexSchemaRegistry',
  'SharedIndexSourceRegistry',
  'SharedIndexMutationEventRegistry',
  'runtime.schemas.registry()',
  'record_retry(METRICS_CONSUMER, STAGE_PROCESS)',
  'StopHandle',
  'tokio::select!',
  'source offset remains uncommitted',
]) {
  requireMarker(host, marker, 'server Product refresh host consumer');
}

for (const forbidden of [
  'move_to_dlq(',
  'legacy',
  'fallback',
  'product.index.product-locale-refresh-v1',
  'rustok-product.product-replay',
]) {
  if (host.includes(forbidden)) {
    fail(`server Product refresh consumer contains forbidden second-path marker: ${forbidden}`);
  }
}

requireMarker(
  bootstrap,
  'product_index_refresh_worker::start_product_index_refresh_worker_if_enabled(&runtime_ctx)',
  'server bootstrap',
);

const applyPosition = genericWorker.indexOf('.apply_replay_mutation(');
const acknowledgePosition = genericWorker.indexOf('.acknowledge(');
if (applyPosition === -1 || acknowledgePosition === -1 || applyPosition > acknowledgePosition) {
  fail('generic source refresh worker no longer preserves durable apply-before-ack ordering');
}

for (const marker of [
  'verify-index-product-refresh-host-consumer.mjs',
  'apps/server/src/services/product_index_refresh_worker.rs',
  'cargo check --locked -p rustok-server --no-default-features --features mod-product --lib',
  'cargo test --locked -p rustok-server --no-default-features --features mod-product product_index_refresh_worker::tests --lib',
]) {
  requireMarker(workflow, marker, 'Index Contract CI');
}

console.log('[verify-index-product-refresh-host-consumer] Product refresh broker/host consumption boundary verified');
