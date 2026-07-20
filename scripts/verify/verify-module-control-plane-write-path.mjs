#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const nonOwnerRoots = [
  'apps/server/src',
  'crates/rustok-installer-persistence/src',
  'crates/rustok-module-build-worker/src',
  'crates/rustok-registry-validation-worker/src',
  'crates/rustok-module-build-transport/src',
  'crates/rustok-verification-worker/src',
  'crates/rustok-verification-transport/src',
  'crates/rustok-module-build-dispatcher/src',
  'crates/rustok-worker-transport/src',
].map((relativePath) => path.join(root, relativePath));
const writePattern = /\b(?:INSERT\s+INTO|UPDATE|DELETE\s+FROM)\s+(?:platform_state|module_operations|tenant_modules|module_artifact_[a-z_]+|module_build_requests|registry_[a-z_]+)\b/i;
const activeModelPattern = /\b(?:module_operations|tenant_modules|module_artifact_[a-z_]+|module_build_requests|registry_[a-z_]+)::ActiveModel\b/;
const ownerServiceConstructorPattern = /\b(?:ModuleDefinitionCatalog::from_static_registry|ModuleEffectivePolicyQuery::new|ModuleLifecycleDbWriter::new|SeaOrmArtifactInstallationStore::new|SeaOrmArtifactSandboxPolicyResolver::new|SeaOrmArtifactDataCapabilityBrokerResolver::new|SeaOrmArtifactDataObjectCapabilityBrokerResolver::new|SeaOrmArtifactDataExportService::new|SeaOrmArtifactSecretService::new|SeaOrmArtifactSecretHandleService::new|SeaOrmArtifactSecretCapabilityBroker::new|SeaOrmArtifactSecretCapabilityBrokerResolver::new|SeaOrmArtifactExecutionObserver::new|SeaOrmArtifactEventSubscriptionProjector::new|SeaOrmArtifactBindingIdempotencyStore::new|SeaOrmModuleBuildService::new|SeaOrmModuleCompositionService::new|SeaOrmModuleGovernanceService::new)\s*\(/;
const ownerBoundaries = [
  {
    path: 'crates/rustok-modules/src/composition.rs',
    pattern: /\b(?:INSERT\s+INTO|UPDATE|DELETE\s+FROM)\s+platform_state\b/i,
  },
  {
    path: 'crates/rustok-modules/src/operation_store.rs',
    pattern: /\b(?:INSERT\s+INTO|UPDATE|DELETE\s+FROM)\s+(?:module_operations|tenant_modules)\b/i,
  },
  {
    path: 'crates/rustok-modules/src/installation.rs',
    pattern: /\b(?:INSERT\s+INTO|UPDATE|DELETE\s+FROM)\s+module_artifact_[a-z_]+\b/i,
  },
  {
    path: 'crates/rustok-modules/src/data.rs',
    pattern: /\b(?:INSERT\s+INTO|UPDATE|DELETE\s+FROM)\s+module_artifact_data[a-z_]*\b/i,
  },
  {
    path: 'crates/rustok-modules/src/build.rs',
    pattern: /\b(?:INSERT\s+INTO|UPDATE|DELETE\s+FROM)\s+module_build_requests\b/i,
  },
  {
    path: 'crates/rustok-modules/src/governance.rs',
    pattern: /\b(?:INSERT\s+INTO|UPDATE|DELETE\s+FROM)\s+registry_[a-z_]+\b/i,
  },
];

function fail(message) {
  throw new Error(`[verify-module-control-plane-write-path] ${message}`);
}

function rustFiles(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) return rustFiles(entryPath);
    return entry.isFile() && entry.name.endsWith('.rs') ? [entryPath] : [];
  });
}

function relative(filePath) {
  return path.relative(root, filePath).replaceAll(path.sep, '/');
}

function writesControlPlane(source) {
  return writePattern.test(source) || activeModelPattern.test(source);
}

function constructsOwnerService(source) {
  return ownerServiceConstructorPattern.test(source);
}

function isProductionSource(filePath) {
  const file = relative(filePath);
  return !file.includes('/tests/') && !file.endsWith('/tests.rs');
}

try {
  const productionSources = nonOwnerRoots
    .flatMap((directory) => rustFiles(directory))
    .filter((filePath) => !relative(filePath).startsWith('apps/server/src/models/'))
    .filter(isProductionSource);
  const writeViolations = productionSources
    .filter((filePath) => writesControlPlane(fs.readFileSync(filePath, 'utf8')))
    .map(relative);
  const constructionViolations = productionSources
    .filter((filePath) => constructsOwnerService(fs.readFileSync(filePath, 'utf8')))
    .map(relative);

  if (writeViolations.length > 0) {
    fail(`control-plane writes must be owner-owned; found: ${writeViolations.join(', ')}`);
  }

  if (constructionViolations.length > 0) {
    fail(
      `control-plane services must be obtained through ModuleControlPlane; found: ${constructionViolations.join(', ')}`,
    );
  }

  for (const owner of ownerBoundaries) {
    const source = fs.readFileSync(path.join(root, owner.path), 'utf8');
    if (!owner.pattern.test(source)) {
      fail(`owner write implementation is missing: ${owner.path}`);
    }
  }

  const bindingStore = fs.readFileSync(
    path.join(root, 'crates/rustok-modules/src/binding_idempotency.rs'),
    'utf8',
  );
  const bindingRlsMigration = fs.readFileSync(
    path.join(
      root,
      'crates/rustok-modules/src/migrations/m20260720_000032_artifact_binding_operation_rls.rs',
    ),
    'utf8',
  );
  const tenantScopeCalls = bindingStore.match(/configure_tenant_scope\s*\(/g)?.length ?? 0;
  if (tenantScopeCalls < 3) {
    fail('artifact binding claim, completion, and abandonment must set transaction-local tenant scope');
  }
  if (
    !bindingRlsMigration.includes('module_artifact_binding_operations_scope') ||
    !bindingRlsMigration.includes("current_setting('rustok.tenant_id', true)")
  ) {
    fail('artifact binding operation persistence must keep its PostgreSQL tenant RLS policy');
  }

  console.log('[verify-module-control-plane-write-path] owner boundaries verified');
} catch (error) {
  if (
    error instanceof Error &&
    error.message.startsWith('[verify-module-control-plane-write-path]')
  ) {
    console.error(error.message);
    process.exit(1);
  }
  throw error;
}
