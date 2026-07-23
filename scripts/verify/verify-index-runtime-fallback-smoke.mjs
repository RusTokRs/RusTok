import { existsSync, readFileSync } from 'node:fs';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const exists = (path) => existsSync(new URL(path, root));
const fail = (message) => {
  console.error(`[verify-index-runtime-fallback-smoke] ${message}`);
  process.exit(1);
};

const nativeAdapter = read('crates/rustok-index/admin/src/transport/native_server_adapter.rs');
const model = read('crates/rustok-index/admin/src/model.rs');
const core = read('crates/rustok-index/admin/src/core.rs');

if (exists('crates/rustok-index/src/ports.rs')) fail('legacy runtime fallback ports must not exist');
for (const table of ['index_content', 'index_products', 'search_index']) {
  if (nativeAdapter.includes(table)) fail(`admin adapter must not read legacy table ${table}`);
}
for (const marker of ['rewrite_status', 'current_milestone']) {
  if (!nativeAdapter.includes(marker) || !model.includes(marker)) {
    fail(`rewrite admin bootstrap missing ${marker}`);
  }
}
if (!core.includes('info_cards')) fail('admin core must expose generic status cards');
if (!nativeAdapter.includes('#[server')) fail('native admin bootstrap must remain available');

console.log('[verify-index-runtime-fallback-smoke] Index admin is detached from legacy tables and exposes rewrite state');
