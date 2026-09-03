import { graphqlRequest, type GqlOpts } from './graphql';
export type { GqlOpts };

export interface ModuleInfo {
  moduleSlug: string;
  name: string;
  description: string;
  version: string;
  kind: 'core' | 'optional';
  dependencies: string[];
  enabled: boolean;
  lifecycleRevision: number;
  ownership: string;
  trustLevel: string;
  recommendedAdminSurfaces: string[];
  showcaseAdminSurfaces: string[];
}

export interface InstalledModule {
  slug: string;
  source: string;
  crateName: string;
  version?: string | null;
  required: boolean;
  dependencies: string[];
}

export interface TenantModule {
  moduleSlug: string;
  enabled: boolean;
  settings: string;
  revision: number;
}

export interface MarketplaceModule {
  slug: string;
  name: string;
  latestVersion: string;
  description: string;
  source: string;
  kind: 'core' | 'optional';
  category: string;
  crateName: string;
  dependencies: string[];
  ownership: string;
  trustLevel: string;
  rustokMinVersion?: string | null;
  rustokMaxVersion?: string | null;
  publisher?: string | null;
  checksumSha256?: string | null;
  signaturePresent: boolean;
  versions: MarketplaceModuleVersion[];
  compatible: boolean;
  recommendedAdminSurfaces: string[];
  showcaseAdminSurfaces: string[];
  installed: boolean;
  installedVersion?: string | null;
  updateAvailable: boolean;
}

export interface MarketplaceModuleVersion {
  version: string;
  changelog?: string | null;
  yanked: boolean;
  publishedAt?: string | null;
  checksumSha256?: string | null;
  signaturePresent: boolean;
}

export interface MarketplaceRegistryFreshness {
  registryId: string;
  status: 'UNKNOWN' | 'READY' | 'DEGRADED';
  lastSuccessUnixMs?: number | null;
  consecutiveFailures: number;
}

export interface BuildJob {
  id: string;
  status: string;
  stage: string;
  progress: number;
  profile: string;
  manifestRef: string;
  manifestHash: string;
  modulesDelta: string;
  requestedBy: string;
  logsUrl?: string | null;
  errorMessage?: string | null;
  reason?: string | null;
  startedAt?: string | null;
  createdAt: string;
  updatedAt: string;
  finishedAt?: string | null;
}

interface BuildOrchestrationSnapshot {
  activeBuild: BuildJob | null;
  buildHistory: BuildJob[];
  marketplaceModules: MarketplaceModule[];
}

export type ModuleTransitionState =
  | 'PREFLIGHTING'
  | 'FENCED'
  | 'PRESTAGING'
  | 'ACTIVATING'
  | 'OBSERVING'
  | 'ROLLBACK_TRIGGERED'
  | 'RECOVERED_TO_PREDECESSOR'
  | 'CONVERGED'
  | 'FAILED_CLOSED';

export interface ModuleTransitionCheckpoint {
  operationId: string;
  moduleSlug: string;
  tenantId?: string | null;
  predecessorDigest?: string | null;
  candidateDigest: string;
  state: ModuleTransitionState;
  stateDetails?: string | null;
  securityEpoch: number;
  recoveryAttemptCount: number;
  createdAt: string;
  updatedAt: string;
}

export interface RetentionHold {
  holdId: string;
  targetType: string;
  targetIdentity: string;
  kind: string;
  createdAt: string;
}

export interface ModuleOperationRecoveryPlan {
  operationId: string;
  tenantId: string;
  moduleSlug: string;
  requestedEnabled: boolean;
  previousEffectiveEnabled: boolean;
  status: string;
  issue: string;
  retryable: boolean;
  recommendedAction: string;
  correlationId: string;
  requestedBy: string;
  errorMessage?: string | null;
}

export interface RegistryMutationResult {
  schema_version: number;
  action: string;
  dry_run: boolean;
  accepted: boolean;
  request_id?: string | null;
  status?: string | null;
  slug: string;
  version: string;
  warnings: string[];
  errors: string[];
  next_step?: string | null;
}

export interface RegistryFollowUpGate {
  key: string;
  status: string;
  detail?: string | null;
  updatedAt: string;
}

export interface RegistryValidationStage {
  key: string;
  status: string;
  detail?: string | null;
  attemptNumber: number;
  updatedAt: string;
  startedAt?: string | null;
  finishedAt?: string | null;
}

export interface RegistryGovernanceAction {
  key: string;
  reasonRequired: boolean;
  reasonCodeRequired: boolean;
  reasonCodes: string[];
  destructive: boolean;
}

export interface RegistryPublishStatusContract {
  schema_version: number;
  request_id: string;
  slug: string;
  version: string;
  status: string;
  accepted: boolean;
  warnings: string[];
  errors: string[];
  followUpGates: RegistryFollowUpGate[];
  validationStages: RegistryValidationStage[];
  approvalOverrideRequired: boolean;
  approvalOverrideReasonCodes: string[];
  governanceActions: RegistryGovernanceAction[];
  next_step?: string | null;
}

export const REGISTRY_OWNER_TRANSFER_REASON_CODES = [
  'maintenance_handoff',
  'team_restructure',
  'publisher_rotation',
  'security_emergency',
  'governance_override',
  'other'
] as const;

export const REGISTRY_YANK_REASON_CODES = [
  'security',
  'legal',
  'malware',
  'critical_regression',
  'rollback',
  'other'
] as const;

const ENABLED_MODULES_QUERY = `
query EnabledModules {
  enabledModules
}
`;

const BUILD_JOB_FIELDS = `
  id
  status
  stage
  progress
  profile
  manifestRef
  manifestHash
  modulesDelta
  requestedBy
  reason
  logsUrl
  errorMessage
  startedAt
  createdAt
  updatedAt
  finishedAt
`;

const MODULE_REGISTRY_QUERY = `
query ModuleRegistry {
  moduleRegistry {
    moduleSlug
    name
    description
    version
    kind
    dependencies
    enabled
    lifecycleRevision
    ownership
    trustLevel
    recommendedAdminSurfaces
    showcaseAdminSurfaces
  }
}
`;

const INSTALLED_MODULES_QUERY = `
query InstalledModules {
  installedModules {
    slug
    source
    crateName
    version
    required
    dependencies
  }
}
`;

const TENANT_MODULES_QUERY = `
query TenantModules {
  tenantModules {
    moduleSlug
    enabled
    settings
    revision
  }
}
`;

const MODULE_COMPOSITION_SNAPSHOT_QUERY = `
query ModuleCompositionSnapshot {
  moduleCompositionSnapshot {
    revision
  }
}
`;

const MARKETPLACE_QUERY = `
query Marketplace(
  $search: String
  $category: String
  $source: String
  $trustLevel: String
  $onlyCompatible: Boolean
  $installedOnly: Boolean
) {
  marketplace(
    search: $search
    category: $category
    source: $source
    trustLevel: $trustLevel
    onlyCompatible: $onlyCompatible
    installedOnly: $installedOnly
  ) {
    slug
    name
    latestVersion
    description
    source
    kind
    category
    crateName
    dependencies
    ownership
    trustLevel
    rustokMinVersion
    rustokMaxVersion
    publisher
    checksumSha256
    signaturePresent
    versions {
      version
      changelog
      yanked
      publishedAt
      checksumSha256
      signaturePresent
    }
    compatible
    recommendedAdminSurfaces
    showcaseAdminSurfaces
    installed
    installedVersion
    updateAvailable
  }
}
`;

const MARKETPLACE_MODULE_QUERY = `
query MarketplaceModule($slug: String!) {
  marketplaceModule(slug: $slug) {
    slug
    name
    latestVersion
    description
    source
    kind
    category
    crateName
    dependencies
    ownership
    trustLevel
    rustokMinVersion
    rustokMaxVersion
    publisher
    checksumSha256
    signaturePresent
    versions {
      version
      changelog
      yanked
      publishedAt
      checksumSha256
      signaturePresent
    }
    compatible
    recommendedAdminSurfaces
    showcaseAdminSurfaces
    installed
    installedVersion
    updateAvailable
  }
}
`;

const MARKETPLACE_REGISTRY_FRESHNESS_QUERY = `
query MarketplaceRegistryFreshness {
  marketplaceRegistryFreshness {
    registryId
    status
    lastSuccessUnixMs
    consecutiveFailures
  }
}
`;

const ACTIVE_BUILD_QUERY = `
query ActiveBuild {
  activeBuild {
${BUILD_JOB_FIELDS}
  }
}
`;

const BUILD_HISTORY_QUERY = `
query BuildHistory($limit: Int!, $offset: Int!) {
  buildHistory(limit: $limit, offset: $offset) {
${BUILD_JOB_FIELDS}
  }
}
`;

const TOGGLE_MODULE_MUTATION = `
mutation ToggleModule($moduleSlug: String!, $enabled: Boolean!, $expectedRevision: Int!, $idempotencyKey: UUID!) {
  toggleModule(moduleSlug: $moduleSlug, enabled: $enabled, expectedRevision: $expectedRevision, idempotencyKey: $idempotencyKey) {
    moduleSlug
    enabled
    settings
    revision
  }
}
`;

const UPDATE_MODULE_SETTINGS_MUTATION = `
mutation UpdateModuleSettings($moduleSlug: String!, $settings: String!, $expectedRevision: Int!, $idempotencyKey: UUID!) {
  updateModuleSettings(moduleSlug: $moduleSlug, settings: $settings, expectedRevision: $expectedRevision, idempotencyKey: $idempotencyKey) {
    moduleSlug
    enabled
    settings
    revision
  }
}
`;

const INSTALL_MODULE_MUTATION = `
mutation InstallModule($slug: String!, $version: String!, $expectedRevision: Int!, $idempotencyKey: UUID!) {
  installModule(slug: $slug, version: $version, expectedRevision: $expectedRevision, idempotencyKey: $idempotencyKey) {
${BUILD_JOB_FIELDS}
  }
}
`;

const UNINSTALL_MODULE_MUTATION = `
mutation UninstallModule($slug: String!, $expectedRevision: Int!, $idempotencyKey: UUID!) {
  uninstallModule(slug: $slug, expectedRevision: $expectedRevision, idempotencyKey: $idempotencyKey) {
${BUILD_JOB_FIELDS}
  }
}
`;

const UPGRADE_MODULE_MUTATION = `
mutation UpgradeModule($slug: String!, $version: String!, $expectedRevision: Int!, $idempotencyKey: UUID!) {
  upgradeModule(slug: $slug, version: $version, expectedRevision: $expectedRevision, idempotencyKey: $idempotencyKey) {
${BUILD_JOB_FIELDS}
  }
}
`;

const TRANSITION_CHECKPOINT_QUERY = `
query GetTransitionCheckpoint($opId: UUID!) {
  moduleTransitionCheckpoint(operationId: $opId) {
    operationId
    moduleSlug
    tenantId
    predecessorDigest
    candidateDigest
    state
    stateDetails
    securityEpoch
    recoveryAttemptCount
    createdAt
    updatedAt
  }
}
`;

const ACTIVE_MODULE_TRANSITIONS_QUERY = `
query ActiveModuleTransitions {
  activeModuleTransitions {
    operationId
    moduleSlug
    tenantId
    predecessorDigest
    candidateDigest
    state
    stateDetails
    securityEpoch
    recoveryAttemptCount
    createdAt
    updatedAt
  }
}
`;

const RETENTION_HOLDS_QUERY = `
query GetRetentionHolds {
  moduleRetentionHolds {
    holdId
    targetType
    targetIdentity
    kind
    createdAt
  }
}
`;

const TRIGGER_RECOVERY_MUTATION = `
mutation TriggerRecovery($opId: UUID!, $reason: String!) {
  triggerModuleRecovery(operationId: $opId, reason: $reason) {
    operationId
    moduleSlug
    tenantId
    predecessorDigest
    candidateDigest
    state
    stateDetails
    securityEpoch
    recoveryAttemptCount
    createdAt
    updatedAt
  }
}
`;

const FINALIZE_TRANSITION_MUTATION = `
mutation FinalizeTransition($opId: UUID!) {
  finalizeModuleTransition(operationId: $opId) {
    operationId
    moduleSlug
    tenantId
    predecessorDigest
    candidateDigest
    state
    stateDetails
    securityEpoch
    recoveryAttemptCount
    createdAt
    updatedAt
  }
}
`;

const MODULE_OPERATION_RECOVERY_PLAN_QUERY = `
query ModuleOperationRecoveryPlan($operationId: UUID!) {
  moduleOperationRecoveryPlan(operationId: $operationId) {
    operationId
    tenantId
    moduleSlug
    requestedEnabled
    previousEffectiveEnabled
    status
    issue
    retryable
    recommendedAction
    correlationId
    requestedBy
    errorMessage
  }
}
`;

const FAILED_MODULE_OPERATION_RECOVERY_PLANS_QUERY = `
query FailedModuleOperationRecoveryPlans($moduleSlug: String, $limit: Int) {
  failedModuleOperationRecoveryPlans(moduleSlug: $moduleSlug, limit: $limit) {
    operationId
    tenantId
    moduleSlug
    requestedEnabled
    previousEffectiveEnabled
    status
    issue
    retryable
    recommendedAction
    correlationId
    requestedBy
    errorMessage
  }
}
`;

const RETRY_FAILED_MODULE_OPERATION_POST_HOOK_MUTATION = `
mutation RetryFailedModuleOperationPostHook($operationId: UUID!, $idempotencyKey: UUID!, $expectedRevision: Int!) {
  retryFailedModuleOperationPostHook(operationId: $operationId, idempotencyKey: $idempotencyKey, expectedRevision: $expectedRevision) {
    operationId
    tenantId
    moduleSlug
    requestedEnabled
    previousEffectiveEnabled
    status
    issue
    retryable
    recommendedAction
    correlationId
    requestedBy
    errorMessage
  }
}
`;

const COMPENSATE_FAILED_MODULE_OPERATION_MUTATION = `
mutation CompensateFailedModuleOperation($operationId: UUID!, $idempotencyKey: UUID!, $expectedRevision: Int!) {
  compensateFailedModuleOperation(operationId: $operationId, idempotencyKey: $idempotencyKey, expectedRevision: $expectedRevision) {
    moduleSlug
    enabled
    settings
    revision
  }
}
`;

interface ModuleRegistryResponse {
  moduleRegistry: ModuleInfo[];
}

interface EnabledModulesResponse {
  enabledModules: string[];
}

interface InstalledModulesResponse {
  installedModules: InstalledModule[];
}

interface TenantModulesResponse {
  tenantModules: TenantModule[];
}

interface ModuleCompositionSnapshotResponse {
  moduleCompositionSnapshot: {
    revision: number;
  };
}

interface MarketplaceResponse {
  marketplace: MarketplaceModule[];
}

interface MarketplaceModuleResponse {
  marketplaceModule: MarketplaceModule | null;
}

interface MarketplaceRegistryFreshnessResponse {
  marketplaceRegistryFreshness: MarketplaceRegistryFreshness[];
}

interface ActiveBuildResponse {
  activeBuild: BuildJob | null;
}

interface BuildHistoryResponse {
  buildHistory: BuildJob[];
}

interface ToggleModuleResponse {
  toggleModule: {
    moduleSlug: string;
    enabled: boolean;
    settings: string;
    revision: number;
  };
}

interface UpdateModuleSettingsResponse {
  updateModuleSettings: TenantModule;
}

interface InstallModuleResponse {
  installModule: BuildJob;
}

interface UninstallModuleResponse {
  uninstallModule: BuildJob;
}

interface UpgradeModuleResponse {
  upgradeModule: BuildJob;
}

interface ModuleTransitionCheckpointResponse {
  moduleTransitionCheckpoint: ModuleTransitionCheckpoint | null;
}

interface ActiveModuleTransitionsResponse {
  activeModuleTransitions: ModuleTransitionCheckpoint[];
}

interface ModuleRetentionHoldsResponse {
  moduleRetentionHolds: RetentionHold[];
}

interface TriggerModuleRecoveryResponse {
  triggerModuleRecovery: ModuleTransitionCheckpoint;
}

interface FinalizeModuleTransitionResponse {
  finalizeModuleTransition: ModuleTransitionCheckpoint;
}

interface ModuleOperationRecoveryPlanResponse {
  moduleOperationRecoveryPlan: ModuleOperationRecoveryPlan | null;
}

interface FailedModuleOperationRecoveryPlansResponse {
  failedModuleOperationRecoveryPlans: ModuleOperationRecoveryPlan[];
}

interface RetryFailedModuleOperationPostHookResponse {
  retryFailedModuleOperationPostHook: ModuleOperationRecoveryPlan;
}

interface CompensateFailedModuleOperationResponse {
  compensateFailedModuleOperation: TenantModule;
}

export async function listModules(
  opts: GqlOpts = {}
): Promise<{ modules: ModuleInfo[] }> {
  const data = await graphqlRequest<undefined, ModuleRegistryResponse>(
    MODULE_REGISTRY_QUERY,
    undefined,
    opts.token,
    opts.tenantSlug
  );
  return { modules: data.moduleRegistry };
}

export async function fetchEnabledModules(
  opts: GqlOpts = {}
): Promise<string[]> {
  const data = await graphqlRequest<undefined, EnabledModulesResponse>(
    ENABLED_MODULES_QUERY,
    undefined,
    opts.token,
    opts.tenantSlug
  );

  return data.enabledModules;
}

export async function listInstalledModules(
  opts: GqlOpts = {}
): Promise<InstalledModule[]> {
  const data = await graphqlRequest<undefined, InstalledModulesResponse>(
    INSTALLED_MODULES_QUERY,
    undefined,
    opts.token,
    opts.tenantSlug
  );
  return data.installedModules;
}

export async function listTenantModules(
  opts: GqlOpts = {}
): Promise<TenantModule[]> {
  const data = await graphqlRequest<undefined, TenantModulesResponse>(
    TENANT_MODULES_QUERY,
    undefined,
    opts.token,
    opts.tenantSlug
  );
  return data.tenantModules;
}

export async function getModuleCompositionSnapshot(
  opts: GqlOpts = {}
): Promise<{ revision: number }> {
  const data = await graphqlRequest<undefined, ModuleCompositionSnapshotResponse>(
    MODULE_COMPOSITION_SNAPSHOT_QUERY,
    undefined,
    opts.token,
    opts.tenantSlug
  );
  return data.moduleCompositionSnapshot;
}

export async function listMarketplaceModules(
  search?: string,
  category?: string,
  source?: string,
  trustLevel?: string,
  onlyCompatible?: boolean,
  installedOnly?: boolean,
  opts: GqlOpts = {}
): Promise<MarketplaceModule[]> {
  const data = await graphqlRequest<
    {
      search?: string;
      category?: string;
      source?: string;
      trustLevel?: string;
      onlyCompatible?: boolean;
      installedOnly?: boolean;
    },
    MarketplaceResponse
  >(
    MARKETPLACE_QUERY,
    { search, category, source, trustLevel, onlyCompatible, installedOnly },
    opts.token,
    opts.tenantSlug
  );
  return data.marketplace;
}

export async function getMarketplaceModule(
  slug: string,
  opts: GqlOpts = {}
): Promise<MarketplaceModule | null> {
  const data = await graphqlRequest<
    { slug: string },
    MarketplaceModuleResponse
  >(MARKETPLACE_MODULE_QUERY, { slug }, opts.token, opts.tenantSlug);
  return data.marketplaceModule;
}

export async function listMarketplaceRegistryFreshness(
  opts: GqlOpts = {}
): Promise<MarketplaceRegistryFreshness[]> {
  const data = await graphqlRequest<
    undefined,
    MarketplaceRegistryFreshnessResponse
  >(
    MARKETPLACE_REGISTRY_FRESHNESS_QUERY,
    undefined,
    opts.token,
    opts.tenantSlug
  );
  return data.marketplaceRegistryFreshness;
}

export async function getActiveBuild(
  opts: GqlOpts = {}
): Promise<BuildJob | null> {
  const data = await graphqlRequest<undefined, ActiveBuildResponse>(
    ACTIVE_BUILD_QUERY,
    undefined,
    opts.token,
    opts.tenantSlug
  );
  return data.activeBuild;
}

export async function getBuildHistory(
  limit = 10,
  offset = 0,
  opts: GqlOpts = {}
): Promise<BuildJob[]> {
  const data = await graphqlRequest<
    { limit: number; offset: number },
    BuildHistoryResponse
  >(BUILD_HISTORY_QUERY, { limit, offset }, opts.token, opts.tenantSlug);
  return data.buildHistory;
}

export async function getBuildOrchestrationSnapshot(
  opts: GqlOpts = {}
): Promise<BuildOrchestrationSnapshot> {
  const [activeBuild, buildHistory, marketplaceModules] = await Promise.all([
    getActiveBuild(opts),
    getBuildHistory(10, 0, opts),
    listMarketplaceModules(
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      opts
    )
  ]);

  return {
    activeBuild,
    buildHistory,
    marketplaceModules
  };
}

export async function toggleModule(
  slug: string,
  enabled: boolean,
  expectedRevision: number,
  idempotencyKey: string,
  opts: GqlOpts = {}
): Promise<ModuleInfo> {
  const data = await graphqlRequest<
    {
      moduleSlug: string;
      enabled: boolean;
      expectedRevision: number;
      idempotencyKey: string;
    },
    ToggleModuleResponse
  >(
    TOGGLE_MODULE_MUTATION,
    { moduleSlug: slug, enabled, expectedRevision, idempotencyKey },
    opts.token,
    opts.tenantSlug
  );

  return {
    moduleSlug: data.toggleModule.moduleSlug,
    name: data.toggleModule.moduleSlug,
    description: '',
    version: '',
    kind: 'optional',
    dependencies: [],
    enabled: data.toggleModule.enabled,
    lifecycleRevision: data.toggleModule.revision,
    ownership: 'first_party',
    trustLevel: 'verified',
    recommendedAdminSurfaces: ['leptos-admin', 'next-admin'],
    showcaseAdminSurfaces: []
  };
}

export async function updateModuleSettings(
  slug: string,
  settings: string,
  expectedRevision: number,
  idempotencyKey: string,
  opts: GqlOpts = {}
): Promise<TenantModule> {
  const data = await graphqlRequest<
    {
      moduleSlug: string;
      settings: string;
      expectedRevision: number;
      idempotencyKey: string;
    },
    UpdateModuleSettingsResponse
  >(
    UPDATE_MODULE_SETTINGS_MUTATION,
    { moduleSlug: slug, settings, expectedRevision, idempotencyKey },
    opts.token,
    opts.tenantSlug
  );

  return data.updateModuleSettings;
}

export async function installModule(
  slug: string,
  version: string,
  opts: GqlOpts = {},
  expectedRevision?: number,
  idempotencyKey?: string
): Promise<BuildJob> {
  const rev = expectedRevision ?? (await getModuleCompositionSnapshot(opts)).revision;
  const key = idempotencyKey ?? crypto.randomUUID();
  const data = await graphqlRequest<
    { slug: string; version: string; expectedRevision: number; idempotencyKey: string },
    InstallModuleResponse
  >(
    INSTALL_MODULE_MUTATION,
    { slug, version, expectedRevision: rev, idempotencyKey: key },
    opts.token,
    opts.tenantSlug
  );

  return data.installModule;
}

export async function uninstallModule(
  slug: string,
  opts: GqlOpts = {},
  expectedRevision?: number,
  idempotencyKey?: string
): Promise<BuildJob> {
  const rev = expectedRevision ?? (await getModuleCompositionSnapshot(opts)).revision;
  const key = idempotencyKey ?? crypto.randomUUID();
  const data = await graphqlRequest<
    { slug: string; expectedRevision: number; idempotencyKey: string },
    UninstallModuleResponse
  >(
    UNINSTALL_MODULE_MUTATION,
    { slug, expectedRevision: rev, idempotencyKey: key },
    opts.token,
    opts.tenantSlug
  );

  return data.uninstallModule;
}

export async function upgradeModule(
  slug: string,
  version: string,
  opts: GqlOpts = {},
  expectedRevision?: number,
  idempotencyKey?: string
): Promise<BuildJob> {
  const rev = expectedRevision ?? (await getModuleCompositionSnapshot(opts)).revision;
  const key = idempotencyKey ?? crypto.randomUUID();
  const data = await graphqlRequest<
    { slug: string; version: string; expectedRevision: number; idempotencyKey: string },
    UpgradeModuleResponse
  >(
    UPGRADE_MODULE_MUTATION,
    { slug, version, expectedRevision: rev, idempotencyKey: key },
    opts.token,
    opts.tenantSlug
  );

  return data.upgradeModule;
}

export async function getTransitionCheckpoint(
  operationId: string,
  opts: GqlOpts = {}
): Promise<ModuleTransitionCheckpoint | null> {
  const data = await graphqlRequest<
    { opId: string },
    ModuleTransitionCheckpointResponse
  >(
    TRANSITION_CHECKPOINT_QUERY,
    { opId: operationId },
    opts.token,
    opts.tenantSlug
  );
  return data.moduleTransitionCheckpoint;
}

export async function listActiveModuleTransitions(
  opts: GqlOpts = {}
): Promise<ModuleTransitionCheckpoint[]> {
  const data = await graphqlRequest<undefined, ActiveModuleTransitionsResponse>(
    ACTIVE_MODULE_TRANSITIONS_QUERY,
    undefined,
    opts.token,
    opts.tenantSlug
  );
  return data.activeModuleTransitions;
}

export async function listRetentionHolds(
  opts: GqlOpts = {}
): Promise<RetentionHold[]> {
  const data = await graphqlRequest<undefined, ModuleRetentionHoldsResponse>(
    RETENTION_HOLDS_QUERY,
    undefined,
    opts.token,
    opts.tenantSlug
  );
  return data.moduleRetentionHolds;
}

export async function triggerModuleRecovery(
  operationId: string,
  reason: string,
  opts: GqlOpts = {}
): Promise<ModuleTransitionCheckpoint> {
  const data = await graphqlRequest<
    { opId: string; reason: string },
    TriggerModuleRecoveryResponse
  >(
    TRIGGER_RECOVERY_MUTATION,
    { opId: operationId, reason },
    opts.token,
    opts.tenantSlug
  );
  return data.triggerModuleRecovery;
}

export async function finalizeModuleTransition(
  operationId: string,
  opts: GqlOpts = {}
): Promise<ModuleTransitionCheckpoint> {
  const data = await graphqlRequest<
    { opId: string },
    FinalizeModuleTransitionResponse
  >(
    FINALIZE_TRANSITION_MUTATION,
    { opId: operationId },
    opts.token,
    opts.tenantSlug
  );
  return data.finalizeModuleTransition;
}

export async function getModuleOperationRecoveryPlan(
  operationId: string,
  opts: GqlOpts = {}
): Promise<ModuleOperationRecoveryPlan | null> {
  const data = await graphqlRequest<
    { operationId: string },
    ModuleOperationRecoveryPlanResponse
  >(
    MODULE_OPERATION_RECOVERY_PLAN_QUERY,
    { operationId },
    opts.token,
    opts.tenantSlug
  );
  return data.moduleOperationRecoveryPlan;
}

export async function listFailedModuleOperationRecoveryPlans(
  moduleSlug?: string,
  limit?: number,
  opts: GqlOpts = {}
): Promise<ModuleOperationRecoveryPlan[]> {
  const data = await graphqlRequest<
    { moduleSlug?: string; limit?: number },
    FailedModuleOperationRecoveryPlansResponse
  >(
    FAILED_MODULE_OPERATION_RECOVERY_PLANS_QUERY,
    { moduleSlug, limit },
    opts.token,
    opts.tenantSlug
  );
  return data.failedModuleOperationRecoveryPlans;
}

export async function retryFailedModuleOperationPostHook(
  operationId: string,
  expectedRevision: number,
  idempotencyKey?: string,
  opts: GqlOpts = {}
): Promise<ModuleOperationRecoveryPlan> {
  const key = idempotencyKey ?? crypto.randomUUID();
  const data = await graphqlRequest<
    { operationId: string; expectedRevision: number; idempotencyKey: string },
    RetryFailedModuleOperationPostHookResponse
  >(
    RETRY_FAILED_MODULE_OPERATION_POST_HOOK_MUTATION,
    { operationId, expectedRevision, idempotencyKey: key },
    opts.token,
    opts.tenantSlug
  );
  return data.retryFailedModuleOperationPostHook;
}

export async function compensateFailedModuleOperation(
  operationId: string,
  expectedRevision: number,
  idempotencyKey?: string,
  opts: GqlOpts = {}
): Promise<TenantModule> {
  const key = idempotencyKey ?? crypto.randomUUID();
  const data = await graphqlRequest<
    { operationId: string; expectedRevision: number; idempotencyKey: string },
    CompensateFailedModuleOperationResponse
  >(
    COMPENSATE_FAILED_MODULE_OPERATION_MUTATION,
    { operationId, expectedRevision, idempotencyKey: key },
    opts.token,
    opts.tenantSlug
  );
  return data.compensateFailedModuleOperation;
}

// ---------------------------------------------------------------------------
// Marketplace Registry Governance REST Transport Client
// ---------------------------------------------------------------------------

async function governanceRestRequest<B, T>(
  method: string,
  path: string,
  body?: B,
  opts: GqlOpts = {}
): Promise<T> {
  const base = process.env.NEXT_PUBLIC_API_URL ?? 'http://localhost:5150';
  const url = new URL(path.startsWith('/') ? path : `/${path}`, base);
  const headers: Record<string, string> = {
    'Content-Type': 'application/json'
  };

  if (opts.token) {
    headers['Authorization'] = `Bearer ${opts.token}`;
  }
  if (opts.tenantSlug) {
    headers['X-Tenant-Slug'] = opts.tenantSlug;
  }
  if (opts.tenantId) {
    headers['X-Tenant-ID'] = opts.tenantId;
  }

  const response = await fetch(url.toString(), {
    method,
    headers,
    body: body !== undefined ? JSON.stringify(body) : undefined,
    cache: 'no-store'
  });

  if (!response.ok) {
    const errorText = await response.text();
    let message = `Registry request failed with status ${response.status}`;
    try {
      const parsed = JSON.parse(errorText);
      if (parsed.message) message = parsed.message;
      else if (parsed.error) message = parsed.error;
    } catch {
      if (errorText) message = errorText;
    }
    throw new Error(message);
  }

  return response.json() as Promise<T>;
}

export async function fetchRegistryPublishRequestStatus(
  requestId: string,
  opts: GqlOpts = {}
): Promise<RegistryPublishStatusContract> {
  return governanceRestRequest<undefined, RegistryPublishStatusContract>(
    'GET',
    `/v2/catalog/publish/${requestId}`,
    undefined,
    opts
  );
}

export async function validateRegistryPublishRequest(
  requestId: string,
  dryRun = false,
  opts: GqlOpts = {}
): Promise<RegistryMutationResult> {
  return governanceRestRequest(
    'POST',
    `/v2/catalog/publish/${requestId}/validate`,
    { schema_version: 1, dry_run: dryRun },
    opts
  );
}

export async function approveRegistryPublishRequest(
  requestId: string,
  reason?: string,
  reasonCode?: string,
  dryRun = false,
  opts: GqlOpts = {}
): Promise<RegistryMutationResult> {
  return governanceRestRequest(
    'POST',
    `/v2/catalog/publish/${requestId}/approve`,
    {
      schema_version: 1,
      dry_run: dryRun,
      reason: reason || undefined,
      reason_code: reasonCode || undefined
    },
    opts
  );
}

export async function rejectRegistryPublishRequest(
  requestId: string,
  reason: string,
  reasonCode: string,
  dryRun = false,
  opts: GqlOpts = {}
): Promise<RegistryMutationResult> {
  return governanceRestRequest(
    'POST',
    `/v2/catalog/publish/${requestId}/reject`,
    {
      schema_version: 1,
      dry_run: dryRun,
      reason,
      reason_code: reasonCode
    },
    opts
  );
}

export async function requestChangesRegistryPublishRequest(
  requestId: string,
  reason: string,
  reasonCode: string,
  dryRun = false,
  opts: GqlOpts = {}
): Promise<RegistryMutationResult> {
  return governanceRestRequest(
    'POST',
    `/v2/catalog/publish/${requestId}/request-changes`,
    {
      schema_version: 1,
      dry_run: dryRun,
      reason,
      reason_code: reasonCode
    },
    opts
  );
}

export async function holdRegistryPublishRequest(
  requestId: string,
  reason: string,
  reasonCode: string,
  dryRun = false,
  opts: GqlOpts = {}
): Promise<RegistryMutationResult> {
  return governanceRestRequest(
    'POST',
    `/v2/catalog/publish/${requestId}/hold`,
    {
      schema_version: 1,
      dry_run: dryRun,
      reason,
      reason_code: reasonCode
    },
    opts
  );
}

export async function resumeRegistryPublishRequest(
  requestId: string,
  reason: string,
  reasonCode: string,
  dryRun = false,
  opts: GqlOpts = {}
): Promise<RegistryMutationResult> {
  return governanceRestRequest(
    'POST',
    `/v2/catalog/publish/${requestId}/resume`,
    {
      schema_version: 1,
      dry_run: dryRun,
      reason,
      reason_code: reasonCode
    },
    opts
  );
}

export async function transferRegistryOwner(
  slug: string,
  newOwnerUserId: string,
  reason: string,
  reasonCode: string,
  dryRun = false,
  opts: GqlOpts = {}
): Promise<RegistryMutationResult> {
  return governanceRestRequest(
    'POST',
    '/v2/catalog/owner-transfer',
    {
      schema_version: 1,
      dry_run: dryRun,
      slug,
      new_owner_user_id: newOwnerUserId,
      reason,
      reason_code: reasonCode
    },
    opts
  );
}

export async function yankRegistryRelease(
  slug: string,
  version: string,
  reason: string,
  reasonCode: string,
  dryRun = false,
  opts: GqlOpts = {}
): Promise<RegistryMutationResult> {
  return governanceRestRequest(
    'POST',
    '/v2/catalog/yank',
    {
      schema_version: 1,
      dry_run: dryRun,
      slug,
      version,
      reason,
      reason_code: reasonCode
    },
    opts
  );
}
