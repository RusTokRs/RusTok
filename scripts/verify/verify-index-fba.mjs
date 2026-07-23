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
const manifest = read('crates/rustok-index/rustok-module.toml');
const plan = read('crates/rustok-index/docs/implementation-plan.md');

for (const obsolete of [
  'crates/rustok-index/src/ports.rs',
  'crates/rustok-index/src/models.rs',
  'crates/rustok-index/src/content/query.rs',
  'crates/rustok-index/src/product/query.rs',
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
if (manifest.includes('[fba.provider]')) fail('legacy FBA provider metadata must not return');
if (!plan.includes('- FBA status: `in_progress`')) fail('plan must keep FBA status in_progress during rewrite');
if (!plan.includes('Backward compatibility with the rejected implementation is not a goal')) {
  fail('plan must preserve destructive rewrite policy');
}

console.log('[verify-index-fba] Index rewrite boundary is clean and legacy v1 FBA artifacts are absent');
