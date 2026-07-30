#!/usr/bin/env node

import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const rootPath = configuredRoot
  ? path.resolve(configuredRoot)
  : fileURLToPath(new URL('../../', import.meta.url));
const read = (relativePath) => readFileSync(path.join(rootPath, relativePath), 'utf8');

const serverCargo = read('apps/server/Cargo.toml');
const distributionCargo = read('crates/rustok-distribution/Cargo.toml');
const distributionSource = read('crates/rustok-distribution/src/lib.rs');
const commerceCargo = read('crates/rustok-commerce/Cargo.toml');
const providerRuntime = read('apps/server/src/services/commerce_provider_runtime.rs');
const evidence = JSON.parse(
  read('crates/rustok-commerce/contracts/evidence/server-ai-optional-dependency-source.json'),
);

const failures = [];
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const lineStartingWith = (source, prefix, label) => {
  const line = source.split(/\r?\n/).find((candidate) => candidate.trimStart().startsWith(prefix));
  if (!line) failures.push(`${label}: line starting with ${prefix} not found`);
  return line ?? '';
};

const serverModAi = lineStartingWith(serverCargo, 'mod-ai', 'server mod-ai feature');
const serverModCommerce = lineStartingWith(serverCargo, 'mod-commerce', 'server mod-commerce feature');
const serverAiDependency = lineStartingWith(serverCargo, 'rustok-ai =', 'server AI dependency');
const distributionModAi = lineStartingWith(distributionCargo, 'mod-ai', 'distribution mod-ai feature');
const distributionAiDependency = lineStartingWith(
  distributionCargo,
  'rustok-ai =',
  'distribution AI dependency',
);
const commerceAiDependency = commerceCargo
  .split(/\r?\n/)
  .find((candidate) => candidate.trimStart().startsWith('rustok-ai'));

requireText(serverModAi, 'dep:rustok-ai', 'server mod-ai dependency ownership');
requireText(serverModAi, 'rustok-distribution/mod-ai', 'server distribution AI feature');
requireText(serverAiDependency, 'optional = true', 'server optional AI dependency');
requireText(serverAiDependency, 'features = ["graphql"]', 'server AI GraphQL feature');
forbidText(serverModCommerce, 'mod-ai', 'Commerce-to-AI feature edge');
forbidText(serverModCommerce, 'dep:rustok-ai', 'Commerce-to-AI dependency edge');
forbidText(serverModCommerce, 'rustok-distribution/mod-ai', 'Commerce-to-distribution-AI edge');

requireText(distributionModAi, 'dep:rustok-ai', 'distribution mod-ai dependency ownership');
requireText(distributionAiDependency, 'optional = true', 'distribution optional AI dependency');
requireText(distributionSource, '#[cfg(feature = "mod-ai")]', 'distribution AI registration gate');
requireText(distributionSource, 'registry = registry.register(rustok_ai::AiModule);', 'distribution AI registration');
if (commerceAiDependency) {
  failures.push(`Commerce crate must not depend on AI: ${commerceAiDependency.trim()}`);
}

for (const required of [
  '#[cfg(all(feature = "mod-ai", feature = "mod-order"))]',
  'host.with_shared_value(rustok_ai::SharedAiOrderStatusPort(port))',
  '#[cfg(all(feature = "mod-ai", feature = "mod-product"))]',
  'host.with_shared_value(rustok_ai::SharedAiProductCatalogReadPort(',
]) {
  requireText(providerRuntime, required, 'server AI runtime gate');
}

const collectRustFiles = (directory) => {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...collectRustFiles(entryPath));
    else if (entry.isFile() && entry.name.endsWith('.rs')) files.push(entryPath);
  }
  return files;
};
const runtimePath = path.join(rootPath, 'apps/server/src/services/commerce_provider_runtime.rs');
for (const file of collectRustFiles(path.join(rootPath, 'apps/server/src'))) {
  if (path.resolve(file) === path.resolve(runtimePath)) continue;
  const source = readFileSync(file, 'utf8');
  if (source.includes('rustok_ai::')) {
    failures.push(`server AI reference outside the guarded runtime file: ${path.relative(rootPath, file)}`);
  }
}

if (evidence.status !== 'server_ai_optional_dependency_source_unvalidated') {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
for (const [key, expected] of Object.entries({
  ai_dependency_optional: true,
  mod_ai_owns_dependency: true,
  mod_commerce_requires_mod_ai: false,
  non_default_ai_free_profile_source_complete: true,
})) {
  if (evidence.server_feature_contract?.[key] !== expected) {
    failures.push(`evidence server_feature_contract.${key} must be ${expected}`);
  }
}
for (const key of [
  'tests_run',
  'cargo_run',
  'cargo_tree_run',
  'format_run',
  'verifiers_run',
  'workflow_checks_run',
  'ci_run',
  'non_ai_server_compile_proven',
  'dependency_tree_absence_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`evidence validation.${key} must remain false`);
  }
}

if (failures.length > 0) {
  console.error('Server optional AI dependency verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ rustok-server gates rustok-ai behind mod-ai; mod-commerce and rustok-commerce remain AI-free, with execution evidence still open',
);
