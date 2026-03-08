import { graphqlRequest } from './graphql';

export interface GqlOpts {
  token?: string | null;
  tenantSlug?: string | null;
}

const ENABLED_MODULES_QUERY = `
query EnabledModules {
  enabledModules
}
`;

interface EnabledModulesResponse {
  enabledModules: string[];
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
