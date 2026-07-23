import { existsSync, readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const exists = (path) => existsSync(new URL(path, root));
const fail = (message) => {
  console.error(`[verify-index-fba] ${message}`);
  process.exit(1);
};

const lib = read('crates/rustok-index/src/lib.rs');
const domain = read('crates/rustok-index/src/domain/mod.rs');
const cargo = read('crates/rustok-index/Cargo.toml');
const manifest = read('crates/rustok-index/rustok-module.toml');
const plan = read('crates/rustok-index/docs/implementation-plan.md');
const serverDispatcher = read('apps/server/src/services/module_event_dispatcher.rs');

for (const obsolete of [
  'crates/rustok-index/src/ports.rs',
  'crates/rustok-index/src/models.rs',
  'crates/rustok-index/src/error.rs',
  'crates/rustok-index/src/traits.rs',
  'crates/rustok-index/src/content',
  'crates/rustok-index/src/product',
  'crates/rustok-index/src/flex',
  'crates/rustok-index/src/search',
  'crates/rustok-index/src/migrations',
  'crates/rustok-index/contracts/index-fba-registry.json',
  'crates/rustok-index/contracts/evidence/index-contract-test-static-matrix.json',
  'crates/rustok-index/contracts/evidence/index-runtime-fallback-smoke.json',
]) {
  if (exists(obsolete)) fail(`obsolete rewrite artifact still exists: ${obsolete}`);
}

for (const marker of ['pub mod domain;', 'pub use domain::*;']) {
  if (!lib.includes(marker)) fail(`lib.rs missing ${marker}`);
}
for (const marker of ['IndexSchema', 'IndexRecord', 'IndexMutation', 'IndexQuery', 'FilterExpr']) {
  if (!domain.includes(marker)) fail(`domain surface missing ${marker}`);
}
for (const dependency of [
  'rustok-api',
  'rustok-events',
  'rustok-product',
  'rustok-content',
  'rustok-telemetry',
]) {
  if (cargo.includes(dependency)) fail(`Index core must not depend on ${dependency}`);
}
for (const sourceModule of [
  'pub mod content;',
  'pub mod product;',
  'pub mod flex;',
  'pub mod search;',
  'pub mod migrations;',
  'pub mod traits;',
  'pub mod error;',
]) {
  if (lib.includes(sourceModule)) fail(`legacy module export returned: ${sourceModule}`);
}
for (const runtimeMarker of [
  'IndexerRuntimeConfig',
  'content_indexer',
  'product_indexer',
  'flex_indexer',
  'record_index_reindex_runtime_config',
]) {
  if (serverDispatcher.includes(runtimeMarker)) {
    fail(`server dispatcher still contains legacy Index marker: ${runtimeMarker}`);
  }
}
if (manifest.includes('[fba.provider]')) fail('legacy FBA provider metadata must not return');
if (!plan.includes('- FBA status: `in_progress`')) fail('plan must keep FBA status in_progress during rewrite');
if (!plan.includes('Backward compatibility with the rejected implementation is not a goal')) {
  fail('plan must preserve destructive rewrite policy');
}

console.log('[verify-index-fba] Index boundary contains only generic engine core and module metadata');
