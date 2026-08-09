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
const alloyOwnerSourcePath = path.join(ownerRoot, 'governance.rs');
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
  const alloyOwnerSource = fs.readFileSync(alloyOwnerSourcePath, 'utf8');
  const alloyServerImport = fs.readFileSync(alloyServerImportPath, 'utf8');
  const alloyHttpController = fs.readFileSync(alloyHttpControllerPath, 'utf8');
  const alloyGraphqlMutation = fs.readFileSync(alloyGraphqlMutationPath, 'utf8');
  const alloyMcpImport = fs.readFileSync(alloyMcpImportPath, 'utf8');
  const alloyMcpAccess = fs.readFileSync(alloyMcpAccessPath, 'utf8');
  const alloyMcpStdioServer = fs.readFileSync(alloyMcpStdioServerPath, 'utf8');
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
    !alloyOwnerSource.includes('pub parent_release: Option<crate::ArtifactReleaseRef>') ||
    !alloyOwnerSource.includes('active_published_parent_exists') ||
    !alloyOwnerSource.includes('parent_release.digest == command.source_digest') ||
    !alloyOwnerSource.includes('parent_release_from_stage_row') ||
    !alloyOwnerSource.includes('lineage: crate::ArtifactSourceLineage') ||
    !alloyPublicationMigration.includes('CREATE TABLE registry_publish_alloy_staging') ||
    !alloyPublicationMigration.includes('parent_release_digest') ||
    !alloyPublicationMigration.includes('lineage JSONB NOT NULL') ||
    !alloyPublicationMigration.includes('lineage JSON NOT NULL')
  ) {
    fail('imported Alloy forks must retain parent lineage through owner staging and the published artifact contract');
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
