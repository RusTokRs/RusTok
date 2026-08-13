#!/usr/bin/env node

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const templateRoot = path.join(root, 'crates/rustok-module-template');
const source = fs.readFileSync(path.join(templateRoot, 'src/lib.rs'), 'utf8');
const cargoTemplate = fs.readFileSync(
  path.join(templateRoot, 'assets/Cargo.toml.template'),
  'utf8',
);
const readmeTemplate = fs.readFileSync(
  path.join(templateRoot, 'assets/README.md.template'),
  'utf8',
);
const indexIntegrationGuide = fs.readFileSync(
  path.join(templateRoot, 'assets/docs/index-integration.md.template'),
  'utf8',
);
const guestTemplate = fs.readFileSync(
  path.join(templateRoot, 'assets/src/lib.rs.template'),
  'utf8',
);
const sandboxScenario = fs.readFileSync(
  path.join(templateRoot, 'assets/tests/sandbox-scenario.json.template'),
  'utf8',
);
const toolchainTemplate = fs.readFileSync(
  path.join(templateRoot, 'assets/rust-toolchain.toml.template'),
  'utf8',
);
const buildContract = fs.readFileSync(
  path.join(root, 'crates/rustok-modules/src/build.rs'),
  'utf8',
);
const sdkManifest = fs.readFileSync(
  path.join(root, 'crates/rustok-module-sdk/Cargo.toml'),
  'utf8',
);
const nativeIndexGuide = fs.readFileSync(
  path.join(root, 'crates/rustok-index/docs/module-source-integration.md'),
  'utf8',
);

for (const marker of [
  'ModuleArtifactSourceManifest::parse(&bytes)',
  'MODULE_ARTIFACT_SOURCE_MANIFEST_FILE',
  'MODULE_BUILD_COMPONENT_TARGET',
  'MODULE_BUILD_RUNTIME_ABI',
  'rustok_module_sdk::SDK_VERSION',
  'pub const TEMPLATE_VERSION: &str = env!("CARGO_PKG_VERSION")',
  'pub const RUST_TOOLCHAIN: &str = "1.96.0"',
  'LocalSandboxScenario::parse(sandbox_scenario.as_bytes())',
  'INDEX_INTEGRATION_GUIDE_TEMPLATE',
  '"docs/index-integration.md"',
]) {
  assert.ok(source.includes(marker), `template renderer is missing ${marker}`);
}

assert.ok(
  cargoTemplate.includes('edition = "2024"') &&
    cargoTemplate.includes('rustok-module-sdk = { version = "={{sdk_version}}" }') &&
    cargoTemplate.includes('[package.metadata.rustok]') &&
    cargoTemplate.includes('template_version = "{{template_version}}"') &&
    cargoTemplate.includes('crate-type = ["cdylib"]'),
  'generated Cargo manifest must use Rust 2024, exact SDK identity, and cdylib output',
);
assert.ok(
  guestTemplate.includes('"{\\"topic\\":\\"module.{{slug}}.executed\\",\\"payload\\":"') &&
    sandboxScenario.includes('"name": "platform.events"') &&
    sandboxScenario.includes('"topics": ["module.{{slug}}.executed"]') &&
    sandboxScenario.includes('"outcome": "success"'),
  'template event example and local sandbox scenario must share one constrained broker contract',
);
assert.ok(
  toolchainTemplate.includes('targets = ["{{component_target}}"]') &&
    buildContract.includes('MODULE_BUILD_COMPONENT_TARGET: &str = "wasm32-wasip2"'),
  'template and build request must select the canonical native WASI P2 target',
);
assert.ok(
  guestTemplate.includes('rustok_module_sdk::Guest') &&
    guestTemplate.includes('rustok_module_sdk::rustok::module::host::invoke') &&
    guestTemplate.includes('rustok_module_sdk::export!(Module)'),
  'guest template must use only generated SDK bindings and the broker import',
);
assert.ok(
  !guestTemplate.includes('wit_bindgen::generate!') &&
    !cargoTemplate.includes('cargo-component'),
  'template must not duplicate WIT bindings or retain cargo-component',
);

assert.ok(
  readmeTemplate.includes('docs/index-integration.md') &&
    readmeTemplate.includes('not an automatic Index bridge') &&
    guestTemplate.includes('does not register an') &&
    indexIntegrationGuide.includes('does not publish a `platform.index` capability') &&
    /Index integration not yet\s+available/.test(indexIntegrationGuide) &&
    indexIntegrationGuide.includes('crates/rustok-index/docs/module-source-integration.md'),
  'standalone template must explain the fail-closed Index compatibility boundary',
);
assert.ok(
  !cargoTemplate.includes('rustok-index') &&
    !guestTemplate.includes('"platform.index"') &&
    !sandboxScenario.includes('"platform.index"'),
  'standalone component template must not invent a direct rustok-index dependency or capability',
);
assert.ok(
  nativeIndexGuide.includes('register_index_schema_source') &&
    nativeIndexGuide.includes('IndexSource for ClassifiedsIndexSource') &&
    nativeIndexGuide.includes('commit-before-ack') &&
    nativeIndexGuide.includes('writes directly to `index_entities`'),
  'native module guide must retain schema, replay, ingestion, and ownership requirements',
);
assert.match(sdkManifest, /^version = "0\.1\.0"$/m);

console.log(
  '[verify-module-template] canonical standalone template and Index boundary verified',
);
