import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, test } from 'node:test';
import { validateNativeModule } from './verify-native-module-architecture.mjs';

const roots = [];
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});
function put(root, name, text) {
  const full = join(root, name);
  mkdirSync(join(full, '..'), { recursive: true });
  writeFileSync(full, text);
}
function fixture(mode = 'no_ui', db = false) {
  const root = mkdtempSync(join(tmpdir(), 'rustok-native-scaffold-'));
  roots.push(root);
  put(root, 'Cargo.toml', '[package]\nname = "rustok-example"\n');
  let manifest = '[module]\nslug = "example"\nui_classification = "' + mode + '"\n';
  if (mode === 'admin_only' || mode === 'dual_surface') {
    manifest += '\n[provides.admin_ui]\nleptos_crate = "rustok-example-admin"\n';
  }
  if (mode === 'storefront_only' || mode === 'dual_surface') {
    manifest += '\n[provides.storefront_ui]\nleptos_crate = "rustok-example-storefront"\n';
  }
  put(root, 'rustok-module.toml', manifest);
  put(root, 'src/lib.rs', 'mod module;\n#[cfg(test)]\nmod tests;\n');
  put(root, 'src/module.rs', 'impl RusToKModule for ExampleModule {}\n');
  put(root, 'src/tests/mod.rs', 'mod contract;\n');
  put(root, 'src/tests/contract.rs', '#[test] fn contract() {}\n');
  put(root, 'src/domain/mod.rs', 'pub struct State;\n');
  put(root, 'src/services/mod.rs', 'pub struct Service;\n');
  if (db) {
    put(root, 'src/entities/mod.rs', 'pub(crate) struct Record;\n');
    put(root, 'src/migrations/mod.rs', 'pub fn migrations() {}\n');
    put(root, 'src/module.rs', 'impl RusToKModule for ExampleModule {}\nimpl MigrationSource for ExampleModule {}\n');
    put(root, 'src/lib.rs', 'mod module;\nmod entities;\nmod migrations;\n#[cfg(test)] mod tests;\n');
  }
  const uiPackages = mode === 'dual_surface' ? ['admin', 'storefront']
    : mode === 'admin_only' ? ['admin'] : mode === 'storefront_only' ? ['storefront'] : [];
  for (const name of uiPackages) {
    put(root, name + '/Cargo.toml', '[package]\nname = "rustok-example-' + name + '"\n');
    put(root, name + '/src/lib.rs', 'mod core;\nmod transport;\nmod ui;\nmod i18n;\n#[cfg(test)] mod tests;\n');
    put(root, name + '/src/i18n.rs', 'pub fn locale() -> String { "en".to_string() }\n');
    put(root, name + '/src/core/mod.rs', 'pub fn title() -> String { "Example".to_string() }\n');
    put(root, name + '/src/transport/mod.rs', 'mod graphql_adapter;\nmod native_server_adapter;\n');
    put(root, name + '/src/transport/graphql_adapter.rs', 'pub fn graphql() {}\n');
    put(root, name + '/src/transport/native_server_adapter.rs', 'pub fn native() {}\n');
    put(root, name + '/src/ui/mod.rs', 'pub fn render() {}\n');
    put(root, name + '/src/tests/mod.rs', 'mod contract;\n');
    put(root, name + '/src/tests/contract.rs', '#[test] fn contract() {}\n');
  }
  return root;
}
function rejects(root, text) {
  const errors = validateNativeModule(root);
  assert.ok(errors.some(error => error.includes(text)), 'missing "' + text + '"; got:\n' + errors.join('\n'));
}

test('minimal no-ui native source shape passes without invented slots', () => {
  assert.deepEqual(validateNativeModule(fixture()), []);
});
test('DB and every UI mode have coherent physical and manifest boundaries', () => {
  for (const mode of ['no_ui', 'admin_only', 'storefront_only', 'dual_surface']) {
    assert.deepEqual(validateNativeModule(fixture(mode, true)), []);
  }
});
test('missing modules and absent registration are rejected', () => {
  const root = fixture();
  rejects(join(root, 'absent'), 'does not exist');
  put(root, 'src/lib.rs', 'mod tests;\n');
  rejects(root, 'must declare module.rs');
});
test('rejects malformed module identity and crate mismatch', () => {
  const root = fixture();
  put(root, 'rustok-module.toml', '[module]\nslug = "not-kebab"\nui_classification = "no_ui"\n');
  rejects(root, 'snake_case');
  rejects(root, 'does not match module.slug');
});
test('rejects root dumping grounds and missing test wiring', () => {
  const root = fixture();
  put(root, 'src/contract_tests.rs', '#[test] fn old_test() {}\n');
  put(root, 'src/lib.rs', 'mod module;\n');
  rejects(root, 'root admits only lib.rs and module.rs');
  rejects(root, 'must wire the contract tests');
});
test('module composition and persistence entities cannot escape lib.rs', () => {
  const root = fixture(true);
  put(root, 'src/lib.rs', 'mod module;\npub mod entities;\nimpl RusToKModule for ExampleModule {}\n');
  rejects(root, 'persistence entities must remain private');
  rejects(root, 'runtime composition belongs to module.rs');
});
test('entities and migrations are paired, never a phantom DB slot', () => {
  const root = fixture();
  put(root, 'src/entities/mod.rs', 'pub struct Record;\n');
  rejects(root, 'owned entities and migrations must be paired');
});
test('domain cannot use persistence; services cannot use GraphQL adapters', () => {
  const root = fixture();
  put(root, 'src/domain/mod.rs', 'use sea_orm::DatabaseConnection;\n');
  put(root, 'src/services/mod.rs', 'use async_graphql::Context;\n');
  rejects(root, 'domain must not import');
  rejects(root, 'services must not import');
});
test('comments mentioning frameworks do not by themselves break a pure domain', () => {
  const root = fixture();
  put(root, 'src/domain/mod.rs', '// sea_orm:: must not be imported here\npub struct Pure;\n');
  assert.deepEqual(validateNativeModule(root), []);
});
test('oversized source exceeds the unchanged 32 KiB source budget', () => {
  const root = fixture();
  put(root, 'src/domain/big.rs', '// ' + 'a'.repeat(33 * 1024));
  rejects(root, 'exceeds strict 32768-byte limit');
});
test('empty and unknown responsibility slots are rejected', () => {
  const root = fixture();
  put(root, 'src/integrations/mod.rs', '//! Placeholder only.\n');
  put(root, 'src/helpers/mod.rs', 'pub fn helper() {}\n');
  rejects(root, 'empty responsibility slot is forbidden');
  rejects(root, 'unrecognized backend responsibility slot');
});
test('manifest UI declaration must match generated packages', () => {
  const root = fixture();
  put(root, 'rustok-module.toml', '[module]\nslug = "example"\nui_classification = "dual_surface"\n');
  rejects(root, 'declared admin UI missing');
  rejects(root, 'declared storefront UI missing');
});
test('UI package registration cannot silently differ from manifest', () => {
  const root = fixture('admin_only');
  put(root, 'rustok-module.toml', '[module]\nslug = "example"\nui_classification = "admin_only"\n');
  rejects(root, '[provides.admin_ui] registration mismatch');
});
test('FFA core cannot use Leptos and UI cannot bypass transport facade', () => {
  const root = fixture('admin_only');
  put(root, 'admin/src/core/mod.rs', 'use leptos::prelude::*;\n');
  put(root, 'admin/src/ui/mod.rs', 'use crate::transport::graphql_adapter::request;\n');
  rejects(root, 'FFA core must not import');
  rejects(root, 'UI must use the module transport facade');
});
test('missing native transport path and demo-only CRUD fail', () => {
  const root = fixture('storefront_only');
  put(root, 'storefront/src/transport/mod.rs', 'mod graphql_adapter;\n');
  put(root, 'storefront/src/ui/mod.rs', 'let sample_rows = vec![];\n');
  rejects(root, 'must wire both GraphQL and native transport adapters');
  rejects(root, 'synthetic data or log-only CRUD is forbidden');
});
test('CLI reports only static evidence and rejects directory traversal slugs', () => {
  const script = new URL('./verify-native-module-architecture.mjs', import.meta.url).pathname;
  const ok = spawnSync(process.execPath, [script, '--module-root', fixture()], { encoding: 'utf8' });
  assert.equal(ok.status, 0, ok.stderr);
  assert.match(ok.stdout, /static architecture only; no FBA\/FFA runtime verification/);
  const bad = spawnSync(process.execPath, [script, '--module', '../wrong'], { encoding: 'utf8' });
  assert.equal(bad.status, 2);
  assert.match(bad.stderr, /Invalid native module slug/);
});
