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
  'crates/rustok-static-distribution-worker/src',
  'crates/rustok-worker-transport/src',
].map((relativePath) => path.join(root, relativePath));
const ownerRoot = path.join(root, 'crates/rustok-modules/src');
const adminModuleTransportRoot = path.join(root, 'apps/admin/src/features/modules/transport');
const ownerManifestPath = path.join(root, 'crates/rustok-modules/Cargo.toml');
const ownerContractsPath = path.join(root, 'crates/rustok-modules/src/contracts.rs');
const lifecycleOwnerPath = path.join(root, 'crates/rustok-modules/src/installation.rs');
const artifactDataOwnerPath = path.join(root, 'crates/rustok-modules/src/data.rs');
const artifactDataExportMigrationPath = path.join(
  root,
  'crates/rustok-modules/src/migrations/m20260718_000031_artifact_data_exports.rs',
);
const artifactBindingIdempotencyOwnerPath = path.join(
  root,
  'crates/rustok-modules/src/binding_idempotency.rs',
);
const artifactBindingIdempotencyMigrationPath = path.join(
  root,
  'crates/rustok-modules/src/migrations/m20260717_000023_artifact_binding_operations.rs',
);
const serverArtifactBindingPath = path.join(root, 'apps/server/src/services/artifact_binding.rs');
const artifactSettingsRecoveryOwnerPath = path.join(
  root,
  'crates/rustok-modules/src/artifact_settings_recovery.rs',
);
const artifactDataSnapshotOwnerPath = path.join(
  root,
  'crates/rustok-modules/src/data_snapshot.rs',
);
const artifactSecretOwnerPath = path.join(root, 'crates/rustok-modules/src/secrets.rs');
const artifactSecurityStateOwnerPath = path.join(
  root,
  'crates/rustok-modules/src/security_state.rs',
);
const staticPromotionOwnerPath = path.join(root, 'crates/rustok-modules/src/promotion.rs');
const staticDistributionBootstrapOwnerPath = path.join(
  root,
  'crates/rustok-modules/src/distribution_bootstrap.rs',
);
const staticDistributionReleaseOwnerPath = path.join(
  root,
  'crates/rustok-modules/src/distribution_release.rs',
);
const staticDistributionRolloutOwnerPath = path.join(
  root,
  'crates/rustok-modules/src/distribution_rollout.rs',
);
const staticDistributionOwnerPath = path.join(root, 'crates/rustok-modules/src/distribution.rs');
const artifactNodeReconciliationOwnerPath = path.join(
  root,
  'crates/rustok-modules/src/artifact_node_reconciliation.rs',
);
const runtimeManifestPath = path.join(root, 'crates/rustok-runtime/Cargo.toml');
const registryValidationWorkerRoot = path.join(root, 'crates/rustok-registry-validation-worker');
const registryValidationWorkerManifestPath = path.join(registryValidationWorkerRoot, 'Cargo.toml');
const registryValidationWorkerMainPath = path.join(registryValidationWorkerRoot, 'src/main.rs');
const registryValidationWorkerLibraryPath = path.join(registryValidationWorkerRoot, 'src/lib.rs');
const staticDistributionWorkerRoot = path.join(root, 'crates/rustok-static-distribution-worker');
const staticDistributionWorkerManifestPath = path.join(staticDistributionWorkerRoot, 'Cargo.toml');
const publicationEvidencePath = path.join(ownerRoot, 'publication_evidence.rs');
const recoveryPath = path.join(ownerRoot, 'recovery.rs');
const serverLifecyclePath = path.join(root, 'apps/server/src/services/module_lifecycle.rs');
const staticLifecycleWriterPath = path.join(ownerRoot, 'lifecycle_writer.rs');
const staticLifecycleJournalPath = path.join(ownerRoot, 'operation_store.rs');
const alloyOwnerSourcePath = path.join(ownerRoot, 'governance.rs');
const registryPublicationMigrationPath = path.join(
  root,
  'crates/rustok-migrations/src/m20260718_000002_add_registry_publication_idempotency.rs',
);
const registryHttpControllerPath = path.join(
  root,
  'apps/server/src/controllers/marketplace_registry.rs',
);
const alloyServerImportPath = path.join(
  root,
  'apps/server/src/services/registry_governance/alloy_import.rs',
);
const alloyHttpControllerPath = path.join(root, 'crates/alloy/src/controllers/mod.rs');
const alloyGraphqlMutationPath = path.join(root, 'crates/alloy/src/graphql/mutation.rs');
const alloyMcpImportPath = path.join(root, 'crates/rustok-mcp/src/alloy_import.rs');
const alloyMcpAccessPath = path.join(root, 'crates/rustok-mcp/src/access.rs');
const alloyMcpStdioServerPath = path.join(root, 'crates/rustok-mcp/src/server.rs');
const serverMcpControllerPath = path.join(root, 'apps/server/src/controllers/mcp.rs');
const alloySandboxRuntimePath = path.join(root, 'crates/alloy/src/sandbox_request.rs');
const alloyTestRunnerPath = path.join(root, 'crates/alloy/src/runner/test.rs');
const alloyReleaseStagerPath = path.join(root, 'crates/alloy/src/runner/release.rs');
const alloyImportModelPath = path.join(root, 'crates/alloy/src/model/import.rs');
const serverAppRuntimePath = path.join(root, 'apps/server/src/services/app_runtime.rs');
const alloyPublicationMigrationPath = path.join(
  root,
  'crates/rustok-modules/src/migrations/m20260727_000041_registry_release_artifact_contracts.rs',
);
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
const ownerServiceConstructorPattern = /\b(?:ModuleDefinitionCatalog::from_static_registry|ModuleEffectivePolicyQuery::new|ModuleLifecycleDbWriter::new|SeaOrmArtifactInstallationStore::new|SeaOrmArtifactSandboxPolicyResolver::new|SeaOrmArtifactDataCapabilityBrokerResolver::new|SeaOrmArtifactDataObjectCapabilityBrokerResolver::new|SeaOrmArtifactDataExportService::new|SeaOrmArtifactDataSnapshotService::new|SeaOrmArtifactDataSnapshotRetentionService::new|SeaOrmArtifactDataSnapshotCollectionService::new|SeaOrmArtifactSecretService::new|SeaOrmArtifactSecretHandleService::new|SeaOrmArtifactSecretHandlePolicy::new|SeaOrmArtifactSecretCapabilityBroker::new|SeaOrmArtifactSecretCapabilityBrokerResolver::new|SeaOrmArtifactSecretUseService::new|ArtifactMcpCapabilityBrokerResolver::new|SeaOrmArtifactExecutionObserver::new|SeaOrmArtifactEventSubscriptionProjector::new|SeaOrmArtifactBindingIdempotencyStore::new|SeaOrmModuleBuildService::new|SeaOrmModuleCompositionService::new|SeaOrmModuleGovernanceService::new|SeaOrmModulePolicyRevisionConsumer::new|SeaOrmModulePromotionService::with_infrastructure|SeaOrmModuleStaticDistributionService::with_infrastructure|SeaOrmModuleStaticDistributionWorkerService::with_infrastructure|SeaOrmModuleStaticDistributionReleaseService::with_infrastructure)\s*\(/;
const directEventEnvelopePattern = /\bEventEnvelope::new\s*\(/;
const adminBackendLogicPattern = /\b(?:Statement::from|DatabaseBackend::|query_(?:one|all)|std::fs::|tokio::fs::|read_to_string\s*\(|Sha256::|walkdir::|cargo\s+(?:build|metadata)|ModuleBuildService::new|(?:rustok_build::)?BuildService\b)\b/;
const staticDistributionWorkerOwnerTypePattern = /\b(?:ModuleStaticDistribution(?:Rollout(?:Request|Receipt|State|Node(?:Report|Phase|Failure|Receipt))?|Release(?:Activation|Rollback|Revocation)?(?:Command|Receipt|State)?|TopologySnapshot)|SeaOrmModuleStaticDistribution(?:Rollout|Release)Service)\b/;
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

function read(relativePath) {
  return fs.readFileSync(path.join(root, relativePath), 'utf8');
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
  const ownerContracts = fs.readFileSync(ownerContractsPath, 'utf8');
  const lifecycleOwner = fs.readFileSync(lifecycleOwnerPath, 'utf8');
  const artifactDataOwner = fs.readFileSync(artifactDataOwnerPath, 'utf8');
  const artifactDataExportMigration = fs.readFileSync(artifactDataExportMigrationPath, 'utf8');
  const artifactBindingIdempotencyOwner = fs.readFileSync(
    artifactBindingIdempotencyOwnerPath,
    'utf8',
  );
  const artifactBindingIdempotencyMigration = fs.readFileSync(
    artifactBindingIdempotencyMigrationPath,
    'utf8',
  );
  const serverArtifactBinding = fs.readFileSync(serverArtifactBindingPath, 'utf8');
  const artifactSettingsRecoveryOwner = fs.readFileSync(
    artifactSettingsRecoveryOwnerPath,
    'utf8',
  );
  const artifactDataSnapshotOwner = fs.readFileSync(artifactDataSnapshotOwnerPath, 'utf8');
  const artifactSecretOwner = fs.readFileSync(artifactSecretOwnerPath, 'utf8');
  const artifactSecurityStateOwner = fs.readFileSync(artifactSecurityStateOwnerPath, 'utf8');
  const staticPromotionOwner = fs.readFileSync(staticPromotionOwnerPath, 'utf8');
  const staticDistributionBootstrapOwner = fs.readFileSync(
    staticDistributionBootstrapOwnerPath,
    'utf8',
  );
  const staticDistributionReleaseOwner = fs.readFileSync(
    staticDistributionReleaseOwnerPath,
    'utf8',
  );
  const staticDistributionRolloutOwner = fs.readFileSync(
    staticDistributionRolloutOwnerPath,
    'utf8',
  );
  const staticDistributionOwner = fs.readFileSync(staticDistributionOwnerPath, 'utf8');
  const artifactNodeReconciliationOwner = fs.readFileSync(
    artifactNodeReconciliationOwnerPath,
    'utf8',
  );
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

  if (
    !ownerContracts.includes('pub struct ModuleCommandContext') ||
    !ownerContracts.includes('pub actor_id: Uuid,') ||
    !ownerContracts.includes('pub correlation_id: Uuid,') ||
    !ownerContracts.includes('pub idempotency_key: Uuid,') ||
    !ownerContracts.includes('tenant_id.is_some_and(|tenant_id| tenant_id.is_nil())')
  ) {
    fail(
      'module command context must use typed UUID evidence and reject a nil tenant identity; platform scope is represented only by an absent tenant_id',
    );
  }

  const promotionContextFields =
    staticPromotionOwner.match(/pub context: ModuleCommandContext,/g) ?? [];
  if (
    promotionContextFields.length !== 2 ||
    !staticPromotionOwner.includes('valid_platform_command_context') ||
    !staticPromotionOwner.includes('trace_id, correlation_id') ||
    !staticPromotionOwner.includes('event_envelope_for_command(')
  ) {
    fail(
      'static promotion commands must retain a platform-scoped ModuleCommandContext in their durable operation receipt and owner-created outbox events',
    );
  }

  const distributionReleaseContextFields =
    staticDistributionReleaseOwner.match(/pub context: ModuleCommandContext,/g) ?? [];
  if (
    distributionReleaseContextFields.length !== 2 ||
    !staticDistributionReleaseOwner.includes('valid_platform_command_context') ||
    !staticDistributionReleaseOwner.includes('trace_id, correlation_id') ||
    !staticDistributionReleaseOwner.includes('event_envelope_for_command(') ||
    !staticDistributionReleaseOwner.includes('"admit", context, request_digest')
  ) {
    fail(
      'static distribution admission and revocation must retain platform-scoped ModuleCommandContext evidence in their shared durable receipt and owner-created outbox events',
    );
  }

  const distributionRolloutContextFields =
    staticDistributionRolloutOwner.match(/pub context: ModuleCommandContext,/g) ?? [];
  if (
    distributionRolloutContextFields.length !== 2 ||
    !staticDistributionRolloutOwner.includes('command.context.tenant_id.is_some()') ||
    !staticDistributionRolloutOwner.includes('trace_id, correlation_id, created_at') ||
    !staticDistributionRolloutOwner.includes('event_envelope_for_command(') ||
    !staticDistributionRolloutOwner.includes('record.trace_id.as_deref()')
  ) {
    fail(
      'static distribution rollout and recovery commands must retain platform-scoped ModuleCommandContext evidence in durable receipts and owner-created outbox events',
    );
  }

  if (
    !staticDistributionOwner.includes('pub context: ModuleCommandContext,') ||
    !staticDistributionOwner.includes('command.context.tenant_id.is_some()') ||
    !staticDistributionOwner.includes('trace_id, correlation_id, created_at') ||
    !staticDistributionOwner.includes('event_envelope_for_command(')
  ) {
    fail(
      'static distribution build commands must retain platform-scoped ModuleCommandContext evidence in their durable receipt and owner-created outbox event',
    );
  }

  const artifactNodeReconciliationContextFields =
    artifactNodeReconciliationOwner.match(/pub context: ModuleCommandContext,/g) ?? [];
  if (
    artifactNodeReconciliationContextFields.length !== 1 ||
    !artifactNodeReconciliationOwner.includes('valid_platform_command_context') ||
    !artifactNodeReconciliationOwner.includes('trace_id, correlation_id, created_at') ||
    !artifactNodeReconciliationOwner.includes('event_envelope_for_command(') ||
    !artifactNodeReconciliationOwner.includes('context: Option<&ModuleCommandContext>')
  ) {
    fail(
      'artifact-node reconciliation requests must retain a platform-scoped ModuleCommandContext in durable receipts and owner-created outbox events while agent reports keep their bounded mTLS evidence path',
    );
  }

  const distributionBootstrapContextFields =
    staticDistributionBootstrapOwner.match(/pub context: ModuleCommandContext,/g) ?? [];
  if (
    distributionBootstrapContextFields.length !== 1 ||
    !staticDistributionBootstrapOwner.includes('context.tenant_id.is_some()') ||
    !staticDistributionBootstrapOwner.includes('trace_id, correlation_id') ||
    !staticDistributionBootstrapOwner.includes('stored_context != *context')
  ) {
    fail(
      'static distribution bootstrap import must retain and replay a platform-scoped ModuleCommandContext in the shared durable receipt',
    );
  }

  const securityStateContextFields =
    artifactSecurityStateOwner.match(/pub context: ModuleCommandContext,/g) ?? [];
  if (
    securityStateContextFields.length !== 1 ||
    !artifactSecurityStateOwner.includes('valid_platform_command_context') ||
    !artifactSecurityStateOwner.includes('trace_id, correlation_id') ||
    !artifactSecurityStateOwner.includes('event_envelope_for_command(')
  ) {
    fail(
      'global artifact security transitions must retain a platform-scoped ModuleCommandContext in their durable operation receipt and owner-created outbox event',
    );
  }

  const lifecycleContextFields = lifecycleOwner.match(/pub context: ModuleCommandContext,/g) ?? [];
  const commandEventEnvelopes = lifecycleOwner.match(/event_envelope_for_command\(/g) ?? [];
  if (
    lifecycleContextFields.length < 7 ||
    commandEventEnvelopes.length < 6 ||
    !lifecycleOwner.includes('context.tenant_id != scope_tenant_id')
  ) {
    fail(
      'artifact lifecycle commands must carry one scope-matched ModuleCommandContext through durable receipts and owner-created events',
    );
  }

  const admissionCommand = lifecycleOwner.match(
    /pub struct ArtifactAdmissionCommand\s*\{(?<fields>[\s\S]*?)\n\}/,
  );
  if (
    !admissionCommand?.groups?.fields.includes('pub context: ModuleCommandContext,') ||
    lifecycleOwner.includes('pub actor_id: Uuid,\n    pub idempotency_key: Uuid,') ||
    !lifecycleOwner.includes('admission command context tenant does not match installation scope') ||
    !lifecycleOwner.includes('trace_id, correlation_id, request_digest, committed_at') ||
    !lifecycleOwner.includes('event_envelope_for_command(\n                    &command.context,')
  ) {
    fail(
      'artifact admission must retain one scope-matched ModuleCommandContext in its durable idempotency receipt and owner-created outbox event',
    );
  }

  const purgeRequest = artifactDataOwner.match(
    /pub struct ArtifactDataPurgeRequest\s*\{(?<fields>[\s\S]*?)\n\}/,
  );
  if (
    !purgeRequest?.groups?.fields.includes('pub context: ModuleCommandContext,') ||
    !artifactDataOwner.includes('request.context.tenant_id != Some(request.scope.tenant_id)') ||
    !artifactDataOwner.includes('trace_id, correlation_id, reason') ||
    !artifactDataOwner.includes('event_envelope_for_command(')
  ) {
    fail(
      'dynamic artifact data purge must preserve a tenant-matched ModuleCommandContext in its durable receipt and owner-created outbox event',
    );
  }

  const exportRequest = artifactDataOwner.match(
    /pub struct ArtifactDataExportRequest\s*\{(?<fields>[\s\S]*?)\n\}/,
  );
  if (
    !exportRequest?.groups?.fields.includes('pub context: ModuleCommandContext,') ||
    !artifactDataOwner.includes('request.context.tenant_id != Some(request.scope.tenant_id)') ||
    !artifactDataOwner.includes('actor_id, trace_id, correlation_id, idempotency_key') ||
    !artifactDataOwner.includes('event_envelope_for_command(\n                    &request.context,') ||
    !artifactDataExportMigration.includes('trace_id TEXT NOT NULL') ||
    !artifactDataExportMigration.includes('correlation_id UUID NOT NULL') ||
    !artifactDataExportMigration.includes('idempotency_key UUID NOT NULL')
  ) {
    fail(
      'dynamic artifact data export must preserve a tenant-matched ModuleCommandContext in its durable audit row and owner-created outbox event',
    );
  }

  const bindingIdempotencyRequest = artifactBindingIdempotencyOwner.match(
    /pub struct ArtifactBindingIdempotencyRequest\s*\{(?<fields>[\s\S]*?)\n\}/,
  );
  if (
    !bindingIdempotencyRequest?.groups?.fields.includes('pub context: ModuleCommandContext,') ||
    !artifactBindingIdempotencyOwner.includes('stored_trace_id != request.context.trace_id') ||
    !artifactBindingIdempotencyOwner.includes(
      'stored_correlation_id != request.context.correlation_id.to_string()',
    ) ||
    !artifactBindingIdempotencyMigration.includes('idempotency_key UUID NOT NULL') ||
    !artifactBindingIdempotencyMigration.includes('trace_id TEXT NOT NULL') ||
    !artifactBindingIdempotencyMigration.includes('correlation_id UUID NOT NULL') ||
    !serverArtifactBinding.includes('artifact_binding_command_context') ||
    !serverArtifactBinding.includes('idempotency_key: Option<Uuid>')
  ) {
    fail(
      'routed artifact binding idempotency must retain a tenant-matched ModuleCommandContext in its durable replay receipt and server adapter',
    );
  }

  const settingsRecoveryContextFields =
    artifactSettingsRecoveryOwner.match(/pub context: ModuleCommandContext,/g) ?? [];
  const settingsRecoveryEventEnvelopes =
    artifactSettingsRecoveryOwner.match(/event_envelope_for_command\(/g) ?? [];
  if (
    settingsRecoveryContextFields.length < 7 ||
    settingsRecoveryEventEnvelopes.length < 7 ||
    !artifactSettingsRecoveryOwner.includes(
      'valid_command_context(request.tenant_id, &request.context)',
    ) ||
    !artifactSettingsRecoveryOwner.includes(
      'trace_id, correlation_id, idempotency_key',
    ) ||
    !artifactSettingsRecoveryOwner.includes('settings_recovery_collection_work_in')
  ) {
    fail(
      'artifact settings recovery commands must retain tenant-matched ModuleCommandContext evidence in every receipt and preserve it when collection work resumes',
    );
  }

  const snapshotContextFields =
    artifactDataSnapshotOwner.match(/pub context: ModuleCommandContext,/g) ?? [];
  const snapshotEventEnvelopes =
    artifactDataSnapshotOwner.match(/event_envelope_for_command\(/g) ?? [];
  if (
    snapshotContextFields.length < 4 ||
    snapshotEventEnvelopes.length < 4 ||
    !artifactDataSnapshotOwner.includes('valid_command_context') ||
    !artifactDataSnapshotOwner.includes('command_context_from_row') ||
    !artifactDataSnapshotOwner.includes('trace_id, correlation_id, idempotency_key')
  ) {
    fail(
      'artifact data snapshot commands must preserve tenant-matched ModuleCommandContext evidence across staging, receipts, and resumable collection work',
    );
  }

  const secretBindingContextFields =
    artifactSecretOwner.match(/pub context: ModuleCommandContext,/g) ?? [];
  if (
    secretBindingContextFields.length !== 1 ||
    !artifactSecretOwner.includes(
      'valid_command_context(request.scope.tenant_id, &request.context)',
    ) ||
    !artifactSecretOwner.includes('command_context_from_receipt_row') ||
    !artifactSecretOwner.includes('trace_id, correlation_id, idempotency_key') ||
    !artifactSecretOwner.includes('event_envelope_for_command(')
  ) {
    fail(
      'artifact secret binding must retain a tenant-matched ModuleCommandContext in its operation receipt and owner-created outbox event',
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

  const registryValidationWorkerManifest = fs.readFileSync(
    registryValidationWorkerManifestPath,
    'utf8',
  );
  const registryValidationWorkerMain = fs.readFileSync(registryValidationWorkerMainPath, 'utf8');
  const registryValidationWorkerLibrary = fs.readFileSync(
    registryValidationWorkerLibraryPath,
    'utf8',
  );
  const staticDistributionWorkerManifest = fs.readFileSync(
    staticDistributionWorkerManifestPath,
    'utf8',
  );
  const staticDistributionWorkerOwnerTypeViolations = rustFiles(staticDistributionWorkerRoot)
    .filter(isProductionSource)
    .filter((filePath) =>
      staticDistributionWorkerOwnerTypePattern.test(fs.readFileSync(filePath, 'utf8')),
    )
    .map(relative);
  const publicationEvidence = fs.readFileSync(publicationEvidencePath, 'utf8');
  const recoverySource = fs.readFileSync(recoveryPath, 'utf8');
  const serverLifecycleSource = fs.readFileSync(serverLifecyclePath, 'utf8');
  const staticLifecycleWriter = fs.readFileSync(staticLifecycleWriterPath, 'utf8');
  const staticLifecycleJournal = fs.readFileSync(staticLifecycleJournalPath, 'utf8');
  const alloyOwnerSource = fs.readFileSync(alloyOwnerSourcePath, 'utf8');
  const registryPublicationMigration = fs.readFileSync(registryPublicationMigrationPath, 'utf8');
  const registryHttpController = fs.readFileSync(registryHttpControllerPath, 'utf8');
  const alloyServerImport = fs.readFileSync(alloyServerImportPath, 'utf8');
  const alloyHttpController = fs.readFileSync(alloyHttpControllerPath, 'utf8');
  const alloyGraphqlMutation = fs.readFileSync(alloyGraphqlMutationPath, 'utf8');
  const alloyMcpImport = fs.readFileSync(alloyMcpImportPath, 'utf8');
  const alloyMcpAccess = fs.readFileSync(alloyMcpAccessPath, 'utf8');
  const alloyMcpStdioServer = fs.readFileSync(alloyMcpStdioServerPath, 'utf8');
  const alloyMcpScaffold = read('crates/rustok-mcp/src/alloy_scaffold.rs');
  const serverMcpController = fs.readFileSync(serverMcpControllerPath, 'utf8');
  const alloySandboxRuntime = fs.readFileSync(alloySandboxRuntimePath, 'utf8');
  const alloyTestRunner = fs.readFileSync(alloyTestRunnerPath, 'utf8');
  const alloyReleaseStager = fs.readFileSync(alloyReleaseStagerPath, 'utf8');
  const alloyImportModel = fs.readFileSync(alloyImportModelPath, 'utf8');
  const serverAppRuntime = fs.readFileSync(serverAppRuntimePath, 'utf8');
  const alloyPublicationMigration = fs.readFileSync(alloyPublicationMigrationPath, 'utf8');
  for (const dependency of [
    'rustok-build-publication',
    'rustok-verification-transport',
    'rustok-worker-transport',
  ]) {
    if (!new RegExp(`^${dependency.replaceAll('-', '\\-')}(?:\\.workspace)?\\s*=`, 'm').test(registryValidationWorkerManifest)) {
      fail(`registry validation worker is missing production evidence dependency ${dependency}`);
    }
  }
  for (const dependency of ['rustok-server', 'rustok-ai', 'rustok-mcp', 'alloy']) {
    if (new RegExp(`^${dependency.replaceAll('-', '\\-')}(?:\\.workspace)?\\s*=`, 'm').test(registryValidationWorkerManifest)) {
      fail(`registry validation worker must remain independent from product infrastructure: ${dependency}`);
    }
  }
  if (
    !registryValidationWorkerMain.includes('GrpcTrustVerifier::connect_with_tls') ||
    !registryValidationWorkerMain.includes('.check_readiness()') ||
    !registryValidationWorkerMain.includes('CommandRegistryCredentialBroker::new') ||
    !registryValidationWorkerMain.includes('ModulePlatformPublicationEvidenceProducer::new')
  ) {
    fail('registry validation worker must compose the credential-scoped OCI reader and readiness-checked mTLS verifier');
  }

  if (
    !alloyOwnerSource.includes('pub context: ModuleCommandContext,') ||
    !alloyOwnerSource.includes('self.context.tenant_id.is_some()') ||
    !alloyOwnerSource.includes('stored_trace_id != command.context.trace_id') ||
    !alloyOwnerSource.includes('stored_correlation_id != command.context.correlation_id.to_string()') ||
    !registryPublicationMigration.includes('actor_id UUID NOT NULL') ||
    !registryPublicationMigration.includes('trace_id TEXT NOT NULL') ||
    !registryPublicationMigration.includes('correlation_id UUID NOT NULL') ||
    !registryHttpController.includes('registry_platform_command_context(auth, idempotency_key)')
  ) {
    fail(
      'registry publication approval must use a platform-scoped ModuleCommandContext and bind its actor, trace, and correlation identity in the immutable replay receipt',
    );
  }

  for (const operationKind of ['reject', 'request_changes', 'hold', 'resume']) {
    if (!alloyOwnerSource.includes(`operation_kind: "${operationKind}"`)) {
      fail(`registry publish-request review operation is missing durable context receipt coverage: ${operationKind}`);
    }
  }
  if (
    !alloyOwnerSource.includes('registry_publish_request_review_operations') ||
    !alloyOwnerSource.includes('valid_platform_registry_command_context') ||
    !alloyOwnerSource.includes('lock_publish_request') ||
    !alloyOwnerSource.includes('PublishRequestReviewIdempotencyConflict') ||
    !registryPublicationMigration.includes('CREATE TABLE registry_publish_request_review_operations') ||
    !registryPublicationMigration.includes("operation_kind IN ('reject', 'request_changes', 'hold', 'resume')") ||
    !/\.reject_publish_request\(\s*&request_id,\s*&authority,\s*command_context,/s.test(
      registryHttpController,
    ) ||
    !/\.request_changes_publish_request\(\s*&request_id,\s*&authority,\s*command_context,/s.test(
      registryHttpController,
    ) ||
    !/\.hold_publish_request\(\s*&request_id,\s*&authority,\s*command_context,/s.test(
      registryHttpController,
    ) ||
    !/\.resume_publish_request\(\s*&request_id,\s*&authority,\s*command_context,/s.test(
      registryHttpController,
    )
  ) {
    fail(
      'registry publish-request reject, request-changes, hold, and resume commands must use a platform-scoped ModuleCommandContext and one immutable exact-replay receipt ledger',
    );
  }
  if (
    !alloyOwnerSource.includes('pub struct ModuleValidationStageReportCommand') ||
    !alloyOwnerSource.includes('ValidationStageReportIdempotencyConflict') ||
    !alloyOwnerSource.includes('validation_stage_report_replay') ||
    !alloyOwnerSource.includes('record_validation_stage_report_receipt') ||
    !alloyOwnerSource.includes('valid_platform_registry_command_context(&self.context, &self.actor_principal)') ||
    !registryPublicationMigration.includes('CREATE TABLE registry_validation_stage_report_operations') ||
    !/report_validation_stage\([\s\S]*?request\.requeue,\s*command_context,/s.test(
      registryHttpController,
    )
  ) {
    fail(
      'manual registry validation-stage reports must use a platform-scoped ModuleCommandContext and an immutable exact-replay receipt before they mutate the publish-request aggregate',
    );
  }
  if (
    !alloyOwnerSource.includes('registry_validation_job_enqueue_operations') ||
    !alloyOwnerSource.includes('ValidationJobEnqueueIdempotencyConflict') ||
    !alloyOwnerSource.includes('validation_job_enqueue_replay') ||
    !alloyOwnerSource.includes('record_validation_job_enqueue_receipt') ||
    !alloyOwnerSource.includes('valid_platform_registry_command_context(&self.context, &self.actor_principal)') ||
    !registryPublicationMigration.includes('CREATE TABLE registry_validation_job_enqueue_operations') ||
    !/\.validate_publish_request\(\s*&request_id,\s*&authority,\s*command_context,?\s*\)/s.test(
      registryHttpController,
    )
  ) {
    fail(
      'registry validation-job enqueue must use a platform-scoped ModuleCommandContext and durable exact-replay receipt before it mutates the publish-request aggregate',
    );
  }
  if (
    !alloyOwnerSource.includes('registry_release_yank_operations') ||
    !alloyOwnerSource.includes('ReleaseYankIdempotencyConflict') ||
    !alloyOwnerSource.includes('release_yank_replay') ||
    !registryPublicationMigration.includes('CREATE TABLE registry_release_yank_operations') ||
    !registryPublicationMigration.includes("resulting_status = 'yanked'") ||
    !/\.yank_release\(\s*&request\.slug,\s*&request\.version,\s*reason,\s*reason_code,\s*&authority,\s*command_context,/s.test(
      registryHttpController,
    )
  ) {
    fail(
      'registry release yanking must use a platform-scoped ModuleCommandContext and immutable exact-replay receipt ledger',
    );
  }
  if (
    !alloyOwnerSource.includes('registry_owner_transfer_operations') ||
    !alloyOwnerSource.includes('OwnerTransferIdempotencyConflict') ||
    !alloyOwnerSource.includes('owner_transfer_replay') ||
    !registryPublicationMigration.includes('CREATE TABLE registry_owner_transfer_operations') ||
    !/\.transfer_registry_slug_owner\(\s*&request\.slug,[\s\S]*?&authority,\s*command_context,/s.test(registryHttpController)
  ) {
    fail('registry owner transfer must use a platform-scoped ModuleCommandContext and immutable exact-replay receipt ledger');
  }
  if (
    !alloyOwnerSource.includes('registry_publish_request_create_operations') ||
    !alloyOwnerSource.includes('valid_command_context_actor') ||
    !registryPublicationMigration.includes('CREATE TABLE registry_publish_request_create_operations') ||
    !/\.create_publish_request\(&request,\s*&authority,\s*command_context\)/s.test(registryHttpController)
  ) {
    fail('registry publish-request creation must use a typed command context and durable immutable create receipt');
  }
  if (
    !alloyOwnerSource.includes('registry_publish_artifact_operations') ||
    !alloyOwnerSource.includes('PublishArtifactReceipt') ||
    !alloyOwnerSource.includes('publish_artifact_replay') ||
    !alloyOwnerSource.includes('PublishRequestArtifactIdempotencyConflict') ||
    !registryPublicationMigration.includes('CREATE TABLE registry_publish_artifact_operations') ||
    !/\.upload_publish_artifact\(\s*&request_id,\s*&authority,\s*command_context,/s.test(
      registryHttpController,
    )
  ) {
    fail('registry publish-artifact attachment must use a typed command context and durable immutable exact-replay receipt');
  }
  if (
    !registryValidationWorkerLibrary.includes('CredentialedOciRegistryProvider') ||
    !registryValidationWorkerLibrary.includes('.produce(command).await') ||
    !publicationEvidence.includes('.load_source(&command.request_id)') ||
    !publicationEvidence.includes('.fetch(&source.receipt.artifact, self.limits)') ||
    !publicationEvidence.includes('.record_build_service_attestation(') ||
    !publicationEvidence.includes('.record_platform_admission(')
  ) {
    fail('production publication evidence must bind owner staging, exact OCI bytes, isolated verification, and reserved owner records');
  }

  for (const dependency of ['rustok-server', 'sea-orm']) {
    if (new RegExp(`^${dependency.replaceAll('-', '\\-')}(?:\\.workspace)?\\s*=`, 'm').test(staticDistributionWorkerManifest)) {
      fail(`static distribution worker must remain independent from deployment and owner persistence: ${dependency}`);
    }
  }
  if (staticDistributionWorkerOwnerTypeViolations.length > 0) {
    fail(
      `static distribution worker must emit build evidence only and must not own release or rollout state: ${staticDistributionWorkerOwnerTypeViolations.join(', ')}`,
    );
  }

  for (const functionName of [
    'module_operation_recovery_plan',
    'failed_module_operation_recovery_plans',
    'retry_failed_post_hook_operation',
  ]) {
    if (new RegExp(`\\bpub\\s+async\\s+fn\\s+${functionName}\\b`).test(recoverySource)) {
      fail(`lifecycle recovery primitive must remain owner-internal: ${functionName}`);
    }
  }
  if (
    serverLifecycleSource.includes('module_operation_recovery_plan(db, operation_id)') ||
    serverLifecycleSource.includes(
      'failed_module_operation_recovery_plans(db, tenant_id, module_slug)',
    ) ||
    !serverLifecycleSource.includes('.recovery_plan(tenant_id, operation_id)') ||
    !serverLifecycleSource.includes('.failed_recovery_plans(tenant_id, module_slug)')
  ) {
    fail('server lifecycle recovery reads must use tenant-bound ModuleLifecycleDbWriter methods');
  }

  const staticLifecycleContextFields =
    staticLifecycleWriter.match(/pub context: ModuleCommandContext,/g) ?? [];
  if (
    staticLifecycleContextFields.length !== 3 ||
    !staticLifecycleWriter.includes('command.context.tenant_id != Some(command.tenant_id)') ||
    !staticLifecycleWriter.includes('trace_id: Some(command.context.trace_id.clone())') ||
    !staticLifecycleWriter.includes('context: &command.context') ||
    !staticLifecycleJournal.includes('pub trace_id: Option<String>,') ||
    !staticLifecycleJournal.includes('existing.trace_id != request.trace_id') ||
    !serverLifecycleSource.includes('context: ModuleCommandContext')
  ) {
    fail(
      'static lifecycle toggle, recovery, and settings commands must use a tenant-matched ModuleCommandContext and retain its trace/correlation evidence in durable replay identity',
    );
  }

  if (
    !alloyOwnerSource.includes('pub async fn published_rhai_workspace') ||
    !alloyOwnerSource.includes('published_artifact_contracts()') ||
    !alloyOwnerSource.includes('get_verified(&release.digest)') ||
    !alloyOwnerSource.includes('RHAI_WORKSPACE_MEDIA_TYPE') ||
    !alloyOwnerSource.includes('canonical_bytes()')
  ) {
    fail('published Alloy source must be materialized by the module owner from an active contract and verified canonical CAS bytes');
  }
  if (
    !alloyServerImport.includes('ModuleControlPlane::new') ||
    !alloyServerImport.includes('.published_rhai_workspace(release, &blobs)') ||
    !alloyServerImport.includes('.artifact_blob_store(') ||
    /\b(?:ModuleMarketplace(?:Entry|Version)|ModuleMarketplaceContentProjection|OciArtifactReference|upload_artifact)\b/.test(
      alloyServerImport,
    )
  ) {
    fail('server Alloy source adapter must compose only the owner provider and durable artifact store');
  }
  if (
    !alloyHttpController.includes('pub async fn import_published_release') ||
    !alloyHttpController.includes('AlloyReleaseImporter::new') ||
    !alloyHttpController.includes('let actor_id = release_actor(auth, &tenant)?') ||
    !alloyHttpController.includes('"/api/alloy/releases/import"')
  ) {
    fail('Alloy HTTP published-release import must require the release actor and use the owner-backed importer');
  }
  if (
    !alloyGraphqlMutation.includes('async fn import_published_release') ||
    !alloyGraphqlMutation.includes('require_release_admin(ctx).await?') ||
    !alloyGraphqlMutation.includes('AlloyReleaseImporter::new')
  ) {
    fail('Alloy GraphQL published-release import must require the release actor and use the owner-backed importer');
  }

  const alloyMcpImportRequest = alloyMcpImport.match(
    /pub struct AlloyPublishedReleaseImportRequest \{([\s\S]*?)\n\}/,
  )?.[1];
  const alloyMcpImportResponse = alloyMcpImport.match(
    /pub struct AlloyPublishedReleaseImportResponse \{([\s\S]*?)\n\}/,
  )?.[1];
  if (
    !alloyMcpImport.includes('TOOL_ALLOY_IMPORT_PUBLISHED_RELEASE') ||
    !alloyMcpImport.includes('AlloyReleaseImporter::new') ||
    !alloyMcpImportRequest ||
    /\b(?:tenant_id|actor_id)\b/.test(alloyMcpImportRequest) ||
    !alloyMcpImportResponse ||
    /\bpub\s+(?:workspace|source)\b/.test(alloyMcpImportResponse)
  ) {
    fail('remote MCP Alloy import must derive tenant and actor at the host boundary and return only redacted draft identity');
  }
  if (
    !alloyMcpAccess.includes('TOOL_ALLOY_IMPORT_PUBLISHED_RELEASE => vec![') ||
    !alloyMcpAccess.includes('Permission::SCRIPTS_MANAGE.to_string()') ||
    !alloyMcpAccess.includes('Permission::MODULES_MANAGE.to_string()')
  ) {
    fail('remote MCP Alloy import must require both scripts.manage and modules.manage');
  }
  if (
    !serverMcpController.includes('execute_remote_alloy_published_release_import') ||
    !serverMcpController.includes('runtime.0.scoped(tenant_id)') ||
    !serverMcpController.includes('alloy_published_rhai_source_provider_handle(') ||
    !serverMcpController.includes('import_published_release(runtime.storage, source, tenant_id, actor_id, request)') ||
    !serverMcpController.includes('"/api/mcp/runtime/tools/call"') ||
    !serverMcpController.includes('"/api/mcp/runtime/tools/stream"')
  ) {
    fail('remote MCP Alloy import must scope the registry to the durable binding and use the owner-backed source provider');
  }
  if (alloyMcpStdioServer.includes('alloy_import_published_release')) {
    fail('generic stdio MCP must not advertise the tenant-bound published Alloy release import');
  }
  const alloyMcpAuthoring = read('crates/rustok-mcp/src/alloy_authoring.rs');
  const alloyAuthoringService = read('crates/alloy/src/authoring.rs');
  if (
    !alloyMcpAuthoring.includes('REMOTE_ALLOY_AUTHORING_TOOL_NAMES') ||
    !alloyMcpAuthoring.includes('TOOL_ALLOY_CREATE_SCRIPT') ||
    !alloyMcpAuthoring.includes('TOOL_ALLOY_RUN_SCRIPT') ||
    !alloyAuthoringService.includes('pub struct AlloyAuthoringService') ||
    !alloyAuthoringService.includes('pub fn from_scoped(runtime: ScopedAlloyRuntime)') ||
    !alloyAuthoringService.includes('pub struct RedactedAlloyScript') ||
    !alloyAuthoringService.includes('source-redacted') ||
    !alloyAuthoringService.includes('production_scoped_storage_rejects_cross_tenant_authoring') ||
    !serverMcpController.includes('is_remote_alloy_authoring_tool(&input.tool_name)') ||
    !serverMcpController.includes('execute_remote_alloy_authoring') ||
    !serverMcpController.includes('AlloyAuthoringService::from_scoped(runtime.0.scoped(tenant_id))') ||
    !serverMcpController.includes('remote_alloy_authoring_identity') ||
    !serverMcpController.includes('identity.tenant_id.as_deref() != binding.tenant_id.as_deref()') ||
    !serverMcpController.includes('remote_alloy_authoring_identity(&binding).is_err()') ||
    !serverMcpController.includes('fn remote_tool_audit_metadata') ||
    !serverMcpController.includes('"source_bearing_alloy_authoring"')
  ) {
    fail('remote MCP Alloy authoring must use the owner-scoped runtime, redact source responses and audit metadata, and prove tenant isolation');
  }
  if (
    alloyMcpStdioServer.includes('REMOTE_ALLOY_AUTHORING_TOOL_NAMES') ||
    alloyMcpStdioServer.includes('TOOL_ALLOY_CREATE_SCRIPT') ||
    alloyMcpStdioServer.includes('TOOL_ALLOY_RUN_SCRIPT')
  ) {
    fail('generic stdio MCP must not advertise remote-only Alloy authoring tools');
  }
  if (
    alloyMcpScaffold.includes('#[allow(') ||
    alloyMcpScaffold.includes('fn render_graphql_') ||
    alloyMcpScaffold.includes('fn render_controllers_') ||
    /[\u0400-\u04FF]/.test(alloyMcpScaffold)
  ) {
    fail('generic MCP module scaffolding must be English-only, lint-clean, and must not create fake transport handlers');
  }
  if (
    !alloySandboxRuntime.includes('pub trait AlloyImportedDraftPolicyProvider') ||
    !alloySandboxRuntime.includes('async fn policy_for(&self, script: &Script)') ||
    !alloySandboxRuntime.includes('.build_test_with_policy(') ||
    !alloySandboxRuntime.includes('imported Alloy draft parent policy provider is unavailable') ||
    !alloyTestRunner.includes('script.parent_release = lease.source.parent_release.clone()') ||
    !alloyImportModel.includes('descriptor.runtime_abi != rustok_sandbox::RHAI_SANDBOX_RUNTIME_ABI') ||
    !alloyServerImport.includes('ArtifactInstallationResolver::resolve(') ||
    !alloyServerImport.includes('ArtifactSandboxPolicyResolver::resolve(') ||
    !serverAppRuntime.includes('.with_imported_draft_policy_provider(')
  ) {
    fail('imported Alloy drafts must resolve the exact installed parent policy for tests and previews and fail closed without it');
  }
  if (
    !alloyReleaseStager.includes('smoke_script.parent_release = source.parent_release.clone()') ||
    !alloyReleaseStager.includes('parent_release: source.parent_release.clone()') ||
    !alloyReleaseStager.includes('sandbox_scenario_digest: smoke_evidence.scenario_digest') ||
    !alloyOwnerSource.includes('pub parent_release: Option<crate::ArtifactReleaseRef>') ||
    !alloyOwnerSource.includes('pub fn alloy_publication_smoke_scenario_digest') ||
    !alloyOwnerSource.includes('sandbox_scenario_digest') ||
    !alloyOwnerSource.includes('active_published_rhai_parent_exists') ||
    !alloyOwnerSource.includes('parent_release.digest == command.source_digest') ||
    !alloyOwnerSource.includes('parent_release_from_stage_row') ||
    !alloyOwnerSource.includes('lineage: crate::ArtifactSourceLineage') ||
    !alloyPublicationMigration.includes('CREATE TABLE registry_publish_alloy_staging') ||
    !alloyPublicationMigration.includes('parent_release_digest') ||
    !alloyPublicationMigration.includes('lineage JSONB NOT NULL') ||
    !alloyPublicationMigration.includes('lineage JSON NOT NULL')
  ) {
    fail('imported Alloy forks and fixed publication smoke evidence must remain immutable through owner staging and the published artifact contract');
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
