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
const ownerRoot = path.join(root, 'crates/rustok-modules/src');
const adminModuleTransportRoot = path.join(root, 'apps/admin/src/features/modules/transport');
const ownerManifestPath = path.join(root, 'crates/rustok-modules/Cargo.toml');
const runtimeManifestPath = path.join(root, 'crates/rustok-runtime/Cargo.toml');
const forbiddenOwnerDependencies = [
  'alloy',
  'async-graphql',
  'axum',
  'leptos',
  'rustok-ai',
  'rustok-commerce',
  'rustok-mcp',
  'rustok-product',
];
const forbiddenOwnerImportPattern = /\b(?:use|extern\s+crate)\s+(?:alloy|async_graphql|axum|leptos|rustok_ai|rustok_commerce|rustok_mcp|rustok_product)\b/;
const writePattern = /\b(?:INSERT\s+INTO|UPDATE|DELETE\s+FROM)\s+(?:platform_state|module_operations|tenant_modules|module_artifact_[a-z_]+|module_build_requests|module_static_[a-z_]+|registry_[a-z_]+)\b/i;
const activeModelPattern = /\b(?:module_operations|tenant_modules|module_artifact_[a-z_]+|module_build_requests|module_static_[a-z_]+|registry_[a-z_]+)::ActiveModel\b/;
const entityMutationPattern = /\b(?:module_operations|tenant_modules|module_artifact_[a-z_]+|module_build_requests|module_static_[a-z_]+|registry_[a-z_]+)::Entity::(?:insert|insert_many|update_many|delete_many|delete_by_id)\b/;
const ownerServiceConstructorPattern = /\b(?:ModuleDefinitionCatalog::from_static_registry|ModuleEffectivePolicyQuery::new|ModuleLifecycleDbWriter::new|SeaOrmArtifactInstallationStore::new|SeaOrmArtifactSandboxPolicyResolver::new|SeaOrmArtifactDataCapabilityBrokerResolver::new|SeaOrmArtifactDataObjectCapabilityBrokerResolver::new|SeaOrmArtifactDataExportService::new|SeaOrmArtifactDataSnapshotService::new|SeaOrmArtifactDataSnapshotRetentionService::new|SeaOrmArtifactDataSnapshotCollectionService::new|SeaOrmArtifactSecretService::new|SeaOrmArtifactSecretHandleService::new|SeaOrmArtifactSecretCapabilityBroker::new|SeaOrmArtifactSecretCapabilityBrokerResolver::new|SeaOrmArtifactSecretUseService::new|SeaOrmArtifactExecutionObserver::new|SeaOrmArtifactEventSubscriptionProjector::new|SeaOrmArtifactBindingIdempotencyStore::new|SeaOrmModuleBuildService::new|SeaOrmModuleCompositionService::new|SeaOrmModuleGovernanceService::new|SeaOrmModulePromotionService::with_infrastructure|SeaOrmModuleStaticDistributionService::with_infrastructure|SeaOrmModuleStaticDistributionWorkerService::with_infrastructure|SeaOrmModuleStaticDistributionReleaseService::with_infrastructure)\s*\(/;
const directEventEnvelopePattern = /\bEventEnvelope::new\s*\(/;
const adminBackendLogicPattern = /\b(?:Statement::from|DatabaseBackend::|query_(?:one|all)|std::fs::|tokio::fs::|read_to_string\s*\(|Sha256::|walkdir::|cargo\s+(?:build|metadata)|ModuleBuildService::new|(?:rustok_build::)?BuildService\b)\b/;
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
    path: 'crates/rustok-modules/src/data_snapshot.rs',
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
  {
    path: 'crates/rustok-modules/src/promotion.rs',
    pattern: /\b(?:INSERT\s+INTO|UPDATE|DELETE\s+FROM)\s+module_static_[a-z_]+\b/i,
  },
  {
    path: 'crates/rustok-modules/src/distribution.rs',
    pattern: /\b(?:INSERT\s+INTO|UPDATE|DELETE\s+FROM)\s+module_static_distribution_[a-z_]+\b/i,
  },
  {
    path: 'crates/rustok-modules/src/distribution_release.rs',
    pattern: /\b(?:INSERT\s+INTO|UPDATE|DELETE\s+FROM)\s+module_static_distribution_release[a-z_]*\b/i,
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
  return (
    writePattern.test(source) ||
    activeModelPattern.test(source) ||
    entityMutationPattern.test(source)
  );
}

function constructsOwnerService(source) {
  return ownerServiceConstructorPattern.test(source);
}

function isProductionSource(filePath) {
  const file = relative(filePath);
  return !file.includes('/tests/') && !file.endsWith('/tests.rs');
}

try {
  const ownerManifest = fs.readFileSync(ownerManifestPath, 'utf8');
  const runtimeManifest = fs.readFileSync(runtimeManifestPath, 'utf8');
  const forbiddenDependencyViolations = forbiddenOwnerDependencies.filter((dependency) =>
    new RegExp(`^${dependency.replaceAll('-', '\\-')}\\s*=`, 'm').test(ownerManifest),
  );
  const forbiddenImportViolations = rustFiles(ownerRoot)
    .filter(isProductionSource)
    .filter((filePath) =>
      forbiddenOwnerImportPattern.test(fs.readFileSync(filePath, 'utf8')),
    )
    .map(relative);
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
  const directEventEnvelopeViolations = rustFiles(ownerRoot)
    .filter(isProductionSource)
    .filter((filePath) => !relative(filePath).includes('/migrations/'))
    .filter((filePath) => relative(filePath) !== 'crates/rustok-modules/src/infrastructure.rs')
    .filter((filePath) => directEventEnvelopePattern.test(fs.readFileSync(filePath, 'utf8')))
    .map(relative);
  const adminBackendLogicViolations = rustFiles(adminModuleTransportRoot)
    .filter(isProductionSource)
    .filter((filePath) => adminBackendLogicPattern.test(fs.readFileSync(filePath, 'utf8')))
    .map(relative);

  if (forbiddenDependencyViolations.length > 0) {
    fail(
      `modules owner must remain independent from AI, product, commerce, MCP, and host/UI frameworks; dependencies found: ${forbiddenDependencyViolations.join(', ')}`,
    );
  }

  if (forbiddenImportViolations.length > 0) {
    fail(
      `modules owner source must not import AI, product, commerce, MCP, or host/UI frameworks; found: ${forbiddenImportViolations.join(', ')}`,
    );
  }

  if (/rustok-api\s*=\s*\{[^}]*features\s*=\s*\[[^\]]*"server"/s.test(runtimeManifest)) {
    fail(
      'neutral rustok-runtime must not enable rustok-api/server and pull host GraphQL/HTTP frameworks into rustok-modules',
    );
  }

  if (writeViolations.length > 0) {
    fail(`control-plane writes must be owner-owned; found: ${writeViolations.join(', ')}`);
  }

  if (constructionViolations.length > 0) {
    fail(
      `control-plane services must be obtained through ModuleControlPlane; found: ${constructionViolations.join(', ')}`,
    );
  }

  if (directEventEnvelopeViolations.length > 0) {
    fail(
      `control-plane events must use injected identity, time, tenant, and actor metadata; found: ${directEventEnvelopeViolations.join(', ')}`,
    );
  }

  if (adminBackendLogicViolations.length > 0) {
    fail(
      `admin module transport must remain an owner-backed adapter without SQL, filesystem, hashing, dependency, or build logic; found: ${adminBackendLogicViolations.join(', ')}`,
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

  console.log(
    '[verify-module-control-plane-write-path] owner boundaries and dependency isolation verified',
  );
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
