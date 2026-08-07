#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-source-call-timeout] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const timeoutPath = 'crates/rustok-index/src/application/source_timeout.rs';
const timeoutSource = requireMarkers(timeoutPath, [
  'const DEFAULT_INDEX_SOURCE_CALL_TIMEOUT: Duration = Duration::from_secs(30);',
  'const INDEX_SOURCE_SCAN_TIMEOUT_CODE: &str = "index_source_scan_timeout";',
  'const INDEX_SOURCE_LOAD_TIMEOUT_CODE: &str = "index_source_load_timeout";',
  'struct TimedIndexSource<S>',
  'timeout(self.call_timeout, self.inner.scan(request)).await',
  'timeout(self.call_timeout, self.inner.load(request)).await',
  'IndexSourceFailure::retryable(code)',
  'super::source_registry::register_index_source(',
  'TimedIndexSource::new(source, DEFAULT_INDEX_SOURCE_CALL_TIMEOUT)',
  'timed_source_classifies_scan_timeout_as_retryable',
  'timed_source_classifies_targeted_load_timeout_as_retryable',
]);
for (const forbidden of [
  'DatabaseConnection',
  'PostgresMutationStore',
  'index_jobs',
  'index_checkpoints',
  'tokio::spawn',
  'tracing::',
  'format!(',
]) {
  if (timeoutSource.includes(forbidden)) {
    fail(`${timeoutPath} contains forbidden storage, scheduler, or raw-detail marker ${forbidden}`);
  }
}

const applicationPath = 'crates/rustok-index/src/application/mod.rs';
const application = requireMarkers(applicationPath, [
  'mod mutation_event;',
  'mod source_registry;',
  'mod source_timeout;',
  'materialize_index_source_registry,',
  'pub use source_timeout::register_index_source;',
  'register_index_mutation_event,',
]);
if (application.includes('materialize_index_source_registry, register_index_source,')) {
  fail(`${applicationPath} must not bypass the timeout wrapper through the public source-registry export`);
}

const cargoPath = 'crates/rustok-index/Cargo.toml';
const cargo = read(cargoPath);
const dependencies = cargo.slice(
  cargo.indexOf('[dependencies]'),
  cargo.indexOf('[dev-dependencies]'),
);
const devDependencies = cargo.slice(cargo.indexOf('[dev-dependencies]'));
if (!dependencies.includes('tokio.workspace = true')) {
  fail(`${cargoPath} must expose tokio to production source timeout code`);
}
if (devDependencies.includes('tokio.workspace = true')) {
  fail(`${cargoPath} must not retain a duplicate dev-only tokio declaration`);
}

for (const bridgePath of [
  'crates/rustok-distribution/src/channel_index.rs',
  'crates/rustok-distribution/src/product_index/product.rs',
  'crates/rustok-distribution/src/product_variant_index.rs',
]) {
  const bridge = requireMarkers(bridgePath, ['register_index_source(']);
  if (bridge.includes('IndexSourceCatalog::register')) {
    fail(`${bridgePath} bypasses the canonical production timeout registration helper`);
  }
}

requireMarkers('crates/rustok-index/docs/m6-source-call-timeout.md', [
  'Status: `source_complete_owner_execution_pending`',
  'The default source-call deadline is `30 seconds`.',
  '`index_source_scan_timeout`',
  '`index_source_load_timeout`',
  'this source wrapper never extends or heartbeats a job lease',
  'complete in-page interruption/timeouts remains open',
  'maintainer-run',
]);
requireMarkers('crates/rustok-index/docs/README.md', [
  '[M6 Bounded Source-call Timeout](./m6-source-call-timeout.md)',
]);
requireMarkers('crates/rustok-index/docs/implementation-plan.md', [
  '- [ ] Add in-page interruption/timeouts, dry-run, and targeted/full/shadow rebuild modes.',
]);

console.log('[verify-index-source-call-timeout] OK');
