#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relativePath) => fs.readFileSync(path.join(root, relativePath), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-refresh-redelivery-evidence] ${message}`);
  process.exit(1);
};
const requireMarker = (source, marker, label) => {
  if (!source.includes(marker)) fail(`${label} is missing ${marker}`);
};

const harnessPath = 'apps/server/tests/product_index_refresh_redelivery_postgres_iggy.rs';
const contractPath = 'crates/rustok-index/contracts/evidence/product-refresh-postgres-iggy-source.json';
const guidePath = 'crates/rustok-index/docs/m5-product-refresh-postgres-iggy-redelivery-evidence.md';
const runnerPath = '.github/workflows/index-product-refresh-redelivery-evidence.yml';
const harness = read(harnessPath);
const contract = JSON.parse(read(contractPath));
const guide = read(guidePath);
const host = read('apps/server/src/services/product_index_refresh_worker.rs');
const bridge = read('crates/rustok-distribution/src/product_index/refresh_event.rs');
const genericWorker = read('crates/rustok-index/src/application/source_refresh_event.rs');
const sourceWorkflow = read('.github/workflows/index-contract-ci.yml');
const runner = read(runnerPath);

if (contract.status !== 'source_ready_maintainer_execution_pending') {
  fail(`machine contract status must remain source_ready_maintainer_execution_pending, got ${contract.status}`);
}
if (contract.evidence_status !== 'runtime_execution_pending') {
  fail(`machine contract must not claim runtime execution, got ${contract.evidence_status}`);
}
if (contract.test !== harnessPath || contract.verifier !== 'scripts/verify/verify-index-product-refresh-redelivery-evidence.mjs') {
  fail('machine contract source paths do not match the executable evidence boundary');
}
if (contract.manual_workflow !== runnerPath || contract.manual_confirmation !== 'execute') {
  fail('machine contract does not pin the manual external evidence runner');
}
if (contract.database_url_fallback !== null) {
  fail('Product refresh evidence must not admit a generic database URL fallback');
}
if (contract.consumer_group !== 'rustok-product-index-refresh' || contract.topic !== 'domain') {
  fail('machine contract drifted from the production Product refresh Iggy route');
}

for (const marker of [
  'RUSTOK_INDEX_PRODUCT_REFRESH_TEST_DATABASE_URL',
  'RUSTOK_INDEX_PRODUCT_REFRESH_TEST_IGGY_ADDRESS',
  'RUSTOK_INDEX_PRODUCT_REFRESH_TEST_IGGY_USERNAME',
  'RUSTOK_INDEX_PRODUCT_REFRESH_TEST_IGGY_PASSWORD',
  'PRODUCT_INDEX_REFRESH_TOPIC: &str = "domain"',
  'PRODUCT_INDEX_REFRESH_CONSUMER_GROUP: &str = "rustok-product-index-refresh"',
  'IggyTransport::new(',
  '.open_persistent_contract_consumer_group(',
  '.publish_contract(',
  'type Token = ConsumedContractEvent;',
  'ProductIndexRefreshEvent::LocaleRefreshRequested',
  'ProductIndexRefreshEvent::VariantRefreshRequested',
  'ProductIndexRefreshDelivery::locale(',
  'ProductIndexRefreshDelivery::variant(',
  'ProductIndexRefreshDeliveryWorker::new(',
  'PostgresMutationStore::new(',
  'materialize_postgres_index_sources(',
  'materialize_index_source_registry(',
  'materialize_index_mutation_event_registry(',
  'IndexReplayMutationOutcome::Duplicate',
  'IndexSourceRefreshEventProcessError::Acknowledge(_)',
  'IndexSourceRefreshEventProcessError::SourceVersionBehind { .. }',
  'attempted.connector_metadata.ack_token = Some(format!("{exact}-injected-failure"));',
  'assert_applied_inbox_once(',
  'assert_inbox_absent(',
  'FROM index_entities',
  'FROM index_inbox',
  'redelivered.raw_payload() != first_raw.as_slice()',
  'required_offset(&redelivered)? != first_offset',
]) {
  requireMarker(harness, marker, 'Product refresh PostgreSQL/Iggy evidence harness');
}

for (const forbidden of [
  'env::var("DATABASE_URL")',
  '127.0.0.1:8090',
  'move_to_dlq(',
  'RUSTOK_PRODUCT_INDEX_REFRESH_CONSUMER_ENABLED',
  'product.index.product-locale-refresh-v1',
  'rustok-product.product-replay',
]) {
  if (harness.includes(forbidden)) {
    fail(`evidence harness contains forbidden fallback/second-path marker: ${forbidden}`);
  }
}

for (const marker of [
  'source_ready_maintainer_execution_pending',
  'A successful source review or compile is not runtime evidence.',
  'There is intentionally no `DATABASE_URL`',
  'IndexReplayMutationOutcome::Duplicate',
  'SourceVersionBehind',
  'runtime claim remains pending',
  'index-product-refresh-redelivery-evidence.yml',
  'manual workflow source is not runtime evidence',
]) {
  requireMarker(guide, marker, 'Product refresh evidence guide');
}

for (const marker of [
  'PRODUCT_INDEX_REFRESH_TOPIC: &str = "domain"',
  'PRODUCT_INDEX_REFRESH_CONSUMER_GROUP: &str = "rustok-product-index-refresh"',
  'type Token = ConsumedContractEvent;',
  'ProductIndexRefreshEvent::LocaleRefreshRequested',
  'ProductIndexRefreshEvent::VariantRefreshRequested',
  'ProductIndexRefreshDelivery::locale(',
  'ProductIndexRefreshDelivery::variant(',
  'ProductIndexRefreshDeliveryWorker::new(',
  'PostgresMutationStore::new(',
]) {
  requireMarker(host, marker, 'server Product refresh consumer parity');
}

for (const marker of [
  'pub enum ProductIndexRefreshDelivery<T>',
  'pub struct ProductIndexRefreshDeliveryWorker<M, A>',
  'product.index.locale_refresh_requested',
  'product.index.variant_refresh_requested',
]) {
  requireMarker(bridge, marker, 'distribution Product refresh bridge parity');
}

const applyPosition = genericWorker.indexOf('.apply_replay_mutation(');
const acknowledgePosition = genericWorker.indexOf('.acknowledge(');
if (applyPosition === -1 || acknowledgePosition === -1 || applyPosition > acknowledgePosition) {
  fail('generic source refresh worker no longer preserves durable apply-before-ack ordering');
}
for (const marker of [
  'MissingSourceMutation',
  'SourceVersionBehind',
  '.apply_replay_mutation(',
  '.acknowledge(',
]) {
  requireMarker(genericWorker, marker, 'generic source refresh contract');
}

for (const marker of [
  'verify-index-product-refresh-redelivery-evidence.mjs',
  'apps/server/tests/product_index_refresh_redelivery_postgres_iggy.rs',
  'cargo check --locked -p rustok-server --no-default-features --features mod-product --test product_index_refresh_redelivery_postgres_iggy',
  '".github/workflows/index-product-refresh-redelivery-evidence.yml"',
]) {
  requireMarker(sourceWorkflow, marker, 'Index Contract CI evidence admission');
}

for (const marker of [
  'name: Index Product Refresh Redelivery Evidence',
  'workflow_dispatch:',
  'confirmation:',
  '"execute"',
  'permissions:',
  'contents: read',
  'persist-credentials: false',
  'cancel-in-progress: false',
  'RUSTOK_INDEX_PRODUCT_REFRESH_TEST_DATABASE_URL: ${{ secrets.RUSTOK_INDEX_PRODUCT_REFRESH_TEST_DATABASE_URL }}',
  'RUSTOK_INDEX_PRODUCT_REFRESH_TEST_IGGY_ADDRESS: ${{ secrets.RUSTOK_INDEX_PRODUCT_REFRESH_TEST_IGGY_ADDRESS }}',
  'RUSTOK_INDEX_PRODUCT_REFRESH_TEST_IGGY_USERNAME: ${{ secrets.RUSTOK_INDEX_PRODUCT_REFRESH_TEST_IGGY_USERNAME }}',
  'RUSTOK_INDEX_PRODUCT_REFRESH_TEST_IGGY_PASSWORD: ${{ secrets.RUSTOK_INDEX_PRODUCT_REFRESH_TEST_IGGY_PASSWORD }}',
  'if [[ "${{ inputs.confirmation }}" != "execute" ]]; then',
  'RUSTOK_INDEX_PRODUCT_REFRESH_TEST_DATABASE_URL secret is required.',
  'RUSTOK_INDEX_PRODUCT_REFRESH_TEST_IGGY_ADDRESS secret is required.',
  'Iggy username/password must either both be configured or both be omitted.',
  'node scripts/verify/verify-index-product-refresh-redelivery-evidence.mjs',
  'cargo test --locked -p rustok-server',
  '--no-default-features --features mod-product',
  '--test product_index_refresh_redelivery_postgres_iggy',
  '-- --nocapture --test-threads=1',
]) {
  requireMarker(runner, marker, 'manual Product refresh PostgreSQL/Iggy evidence runner');
}

for (const forbidden of [
  'pull_request:',
  'push:',
  'contents: write',
  'pull-requests: write',
  'persist-credentials: true',
  'RUSTOK_PRODUCT_INDEX_REFRESH_CONSUMER_ENABLED',
  'DATABASE_URL:',
  '127.0.0.1:8090',
]) {
  if (runner.includes(forbidden)) {
    fail(`manual evidence runner contains forbidden automatic/fallback marker: ${forbidden}`);
  }
}

console.log('[verify-index-product-refresh-redelivery-evidence] Product refresh PostgreSQL/Iggy redelivery evidence source and manual runner verified');
