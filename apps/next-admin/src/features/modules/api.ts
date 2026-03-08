import { graphqlRequest } from '@/shared/api/graphql';

export interface ModuleInfo {
  moduleSlug: string;
  name: string;
  description: string;
  version: string;
  kind: 'core' | 'optional';
  dependencies: string[];
  enabled: boolean;
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
  releaseId?: string | null;
  logsUrl?: string | null;
  errorMessage?: string | null;
  reason?: string | null;
  startedAt?: string | null;
  createdAt: string;
  updatedAt: string;
  finishedAt?: string | null;
}

export interface ReleaseInfo {
  id: string;
  buildId: string;
  status: string;
  environment: string;
  manifestHash: string;
  modules: string[];
  previousReleaseId?: string | null;
  deployedAt?: string | null;
  rolledBackAt?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface BuildOrchestrationSnapshot {
  activeBuild: BuildJob | null;
  activeRelease: ReleaseInfo | null;
  buildHistory: BuildJob[];
  marketplaceModules: MarketplaceModule[];
}

interface GqlOpts {
  token?: string | null;
  tenantSlug?: string | null;
}

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
  releaseId
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

const ACTIVE_BUILD_QUERY = `
query ActiveBuild {
  activeBuild {
${BUILD_JOB_FIELDS}
  }
}
`;

const ACTIVE_RELEASE_QUERY = `
query ActiveRelease {
  activeRelease {
    id
    buildId
    status
    environment
    manifestHash
    modules
    previousReleaseId
    deployedAt
    rolledBackAt
    createdAt
    updatedAt
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
mutation ToggleModule($moduleSlug: String!, $enabled: Boolean!) {
  toggleModule(moduleSlug: $moduleSlug, enabled: $enabled) {
    moduleSlug
    enabled
    settings
  }
}
`;

const INSTALL_MODULE_MUTATION = `
mutation InstallModule($slug: String!, $version: String!) {
  installModule(slug: $slug, version: $version) {
${BUILD_JOB_FIELDS}
  }
}
`;

const UNINSTALL_MODULE_MUTATION = `
mutation UninstallModule($slug: String!) {
  uninstallModule(slug: $slug) {
${BUILD_JOB_FIELDS}
  }
}
`;

const UPGRADE_MODULE_MUTATION = `
mutation UpgradeModule($slug: String!, $version: String!) {
  upgradeModule(slug: $slug, version: $version) {
${BUILD_JOB_FIELDS}
  }
}
`;

const ROLLBACK_BUILD_MUTATION = `
mutation RollbackBuild($buildId: String!) {
  rollbackBuild(buildId: $buildId) {
${BUILD_JOB_FIELDS}
  }
}
`;

interface ModuleRegistryResponse {
  moduleRegistry: ModuleInfo[];
}

interface InstalledModulesResponse {
  installedModules: InstalledModule[];
}

interface MarketplaceResponse {
  marketplace: MarketplaceModule[];
}

interface MarketplaceModuleResponse {
  marketplaceModule: MarketplaceModule | null;
}

interface ActiveBuildResponse {
  activeBuild: BuildJob | null;
}

interface ActiveReleaseResponse {
  activeRelease: ReleaseInfo | null;
}

interface BuildHistoryResponse {
  buildHistory: BuildJob[];
}

interface ToggleModuleResponse {
  toggleModule: {
    moduleSlug: string;
    enabled: boolean;
    settings: string;
  };
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

interface RollbackBuildResponse {
  rollbackBuild: BuildJob;
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

export async function getActiveRelease(
  opts: GqlOpts = {}
): Promise<ReleaseInfo | null> {
  const data = await graphqlRequest<undefined, ActiveReleaseResponse>(
    ACTIVE_RELEASE_QUERY,
    undefined,
    opts.token,
    opts.tenantSlug
  );
  return data.activeRelease;
}

export async function getBuildHistory(
  limit = 10,
  offset = 0,
  opts: GqlOpts = {}
): Promise<BuildJob[]> {
  const data = await graphqlRequest<
    { limit: number; offset: number },
    BuildHistoryResponse
  >(
    BUILD_HISTORY_QUERY,
    { limit, offset },
    opts.token,
    opts.tenantSlug
  );
  return data.buildHistory;
}

export async function getBuildOrchestrationSnapshot(
  opts: GqlOpts = {}
): Promise<BuildOrchestrationSnapshot> {
  const [activeBuild, activeRelease, buildHistory, marketplaceModules] =
    await Promise.all([
      getActiveBuild(opts),
      getActiveRelease(opts),
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
    activeRelease,
    buildHistory,
    marketplaceModules
  };
}

export async function toggleModule(
  slug: string,
  enabled: boolean,
  opts: GqlOpts = {}
): Promise<ModuleInfo> {
  const data = await graphqlRequest<
    { moduleSlug: string; enabled: boolean },
    ToggleModuleResponse
  >(
    TOGGLE_MODULE_MUTATION,
    { moduleSlug: slug, enabled },
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
    ownership: 'first_party',
    trustLevel: 'verified',
    recommendedAdminSurfaces: ['leptos-admin'],
    showcaseAdminSurfaces: []
  };
}

export async function installModule(
  slug: string,
  version: string,
  opts: GqlOpts = {}
): Promise<BuildJob> {
  const data = await graphqlRequest<
    { slug: string; version: string },
    InstallModuleResponse
  >(
    INSTALL_MODULE_MUTATION,
    { slug, version },
    opts.token,
    opts.tenantSlug
  );

  return data.installModule;
}

export async function uninstallModule(
  slug: string,
  opts: GqlOpts = {}
): Promise<BuildJob> {
  const data = await graphqlRequest<{ slug: string }, UninstallModuleResponse>(
    UNINSTALL_MODULE_MUTATION,
    { slug },
    opts.token,
    opts.tenantSlug
  );

  return data.uninstallModule;
}

export async function upgradeModule(
  slug: string,
  version: string,
  opts: GqlOpts = {}
): Promise<BuildJob> {
  const data = await graphqlRequest<
    { slug: string; version: string },
    UpgradeModuleResponse
  >(
    UPGRADE_MODULE_MUTATION,
    { slug, version },
    opts.token,
    opts.tenantSlug
  );

  return data.upgradeModule;
}

export async function rollbackBuild(
  buildId: string,
  opts: GqlOpts = {}
): Promise<BuildJob> {
  const data = await graphqlRequest<
    { buildId: string },
    RollbackBuildResponse
  >(
    ROLLBACK_BUILD_MUTATION,
    { buildId },
    opts.token,
    opts.tenantSlug
  );

  return data.rollbackBuild;
}
