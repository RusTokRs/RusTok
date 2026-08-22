#!/usr/bin/env node

import fs from 'node:fs';

function read(path) {
  return fs.readFileSync(path, 'utf8');
}

function fail(message) {
  console.error('[verify-forum-translation-host-registration] failed:');
  console.error(`- ${message}`);
  process.exit(1);
}

function requireAll(source, markers, label) {
  for (const marker of markers) {
    if (!source.includes(marker)) {
      fail(`${label} missing ${marker}`);
    }
  }
}

const dispatcherPath = 'apps/server/src/services/module_event_dispatcher.rs';
const appRuntimePath = 'apps/server/src/services/app_runtime.rs';
const modulesPath = 'apps/server/src/modules/mod.rs';
const cargoPath = 'apps/server/Cargo.toml';
const workflowPath = '.github/workflows/forum-translation-host-registration-contract.yml';

const dispatcher = read(dispatcherPath);
const appRuntime = read(appRuntimePath);
const modules = read(modulesPath);
const cargo = read(cargoPath);
const workflow = read(workflowPath);

requireAll(
  dispatcher,
  [
    '#[cfg(feature = "mod-forum")]',
    'rustok_forum::services::ForumCategoryTranslationTargetProvider::new(db.clone())',
    'rustok_translation_targets::register_translation_target_provider(&mut extensions, provider)',
    'Forum category translation target provider registration failed',
    'host_runtime_extensions_register_admin_mutation_providers',
    'descriptor.owner_slug.as_str() == "forum"',
    'descriptor.resource_kind.as_str() == "category"'
  ],
  dispatcherPath
);

requireAll(
  appRuntime,
  [
    'let registry = modules::build_registry();',
    'build_shared_runtime_extensions_with_host_providers(',
    'runtime_ctx.shared_insert(runtime_extensions.clone())'
  ],
  appRuntimePath
);

requireAll(
  modules,
  [
    'pub use rustok_distribution::build_registry;',
    'registry.get("forum").expect("forum module")'
  ],
  modulesPath
);

requireAll(
  cargo,
  [
    '"mod-forum",',
    'mod-forum     = ["dep:rustok-forum", "mod-content", "mod-taxonomy", "mod-page_builder", "rustok-content-orchestration/mod-forum", "rustok-distribution/mod-forum"]'
  ],
  cargoPath
);

requireAll(
  workflow,
  [
    'name: Forum Translation Host Registration Contract',
    'node scripts/verify/verify-forum-translation-host-registration.mjs',
    'cargo test --locked -p rustok-server --no-default-features --features mod-forum host_runtime_extensions_register_admin_mutation_providers -- --nocapture'
  ],
  workflowPath
);

console.log(
  '[verify-forum-translation-host-registration] compiled Forum registry, host Translation provider registration, runtime bootstrap storage, descriptor assertion, and focused CI command are wired'
);
