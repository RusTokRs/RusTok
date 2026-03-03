import { graphqlRequest } from '@/lib/graphql';

export interface ModuleInfo {
  moduleSlug: string;
  name: string;
  description: string;
  version: string;
  kind: 'core' | 'optional';
  dependencies: string[];
  enabled: boolean;
}

interface GqlOpts {
  token?: string | null;
  tenantSlug?: string | null;
}

// ---------- GraphQL ----------

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

// ---------- Response types ----------

interface ModuleRegistryResponse {
  moduleRegistry: ModuleInfo[];
}

interface ToggleModuleResponse {
  toggleModule: {
    moduleSlug: string;
    enabled: boolean;
    settings: string;
  };
}

// ---------- API ----------

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

  // Return a ModuleInfo-compatible object with the toggled state
  // The full module info comes from the registry query; here we get the toggle result
  return {
    moduleSlug: data.toggleModule.moduleSlug,
    name: data.toggleModule.moduleSlug,
    description: '',
    version: '',
    kind: 'optional',
    dependencies: [],
    enabled: data.toggleModule.enabled
  };
}
