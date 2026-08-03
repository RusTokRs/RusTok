#!/usr/bin/env node

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const wit = fs.readFileSync(
  path.join(root, 'crates/rustok-module-sdk/wit/module-runtime.wit'),
  'utf8',
);
const sdk = fs.readFileSync(path.join(root, 'crates/rustok-module-sdk/src/lib.rs'), 'utf8');
const host = fs.readFileSync(path.join(root, 'crates/rustok-sandbox/src/wasm.rs'), 'utf8');
const build = fs.readFileSync(path.join(root, 'crates/rustok-modules/src/build.rs'), 'utf8');

for (const marker of [
  'package rustok:module@1.0.0;',
  'interface host',
  'invoke: func(',
  'world module-runtime',
  'import host;',
  'export run: func(input: string) -> result<string, string>;',
]) {
  assert.ok(wit.includes(marker), `canonical WIT is missing ${marker}`);
}

for (const marker of [
  'wit_bindgen::generate!',
  'path: "wit"',
  'world: "module-runtime"',
  'pub_export_macro: true',
]) {
  assert.ok(sdk.includes(marker), `guest SDK generation is missing ${marker}`);
}

assert.ok(
  host.includes('path: "../rustok-module-sdk/wit"') &&
    host.includes('world: "module-runtime"'),
  'host bindings must be generated from the canonical SDK WIT',
);
assert.ok(!host.includes('inline:'), 'host must not retain a duplicate inline WIT contract');
assert.ok(
  build.includes('MODULE_BUILD_WIT_WORLD: &str = "rustok:module/module-runtime"') &&
    build.includes('MODULE_BUILD_WIT_VERSION: &str = "1.0.0"') &&
    build.includes('MODULE_BUILD_COMPONENT_TARGET: &str = "wasm32-wasip2"'),
  'build protocol must select the exact canonical SDK world, version, and native component target',
);

console.log('[verify-module-sdk-wit] canonical generated guest/host WIT binding verified');
