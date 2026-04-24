import { graphqlRequest } from './graphql';

export type SeoTargetCapabilityKind =
  | 'AUTHORING'
  | 'ROUTING'
  | 'BULK'
  | 'SITEMAPS';

export interface SeoTargetCapabilities {
  authoring: boolean;
  routing: boolean;
  bulk: boolean;
  sitemaps: boolean;
}

export interface SeoTargetRegistryEntry {
  slug: string;
  displayName: string;
  ownerModuleSlug: string;
  capabilities: SeoTargetCapabilities;
}

export interface SeoApiOptions {
  token?: string | null;
  tenantSlug?: string | null;
  graphqlUrl?: string;
}

interface SeoTargetsResponse {
  seoTargets: SeoTargetRegistryEntry[];
}

interface SeoTargetsVariables {
  capability?: SeoTargetCapabilityKind | null;
}

const SEO_TARGETS_QUERY = `
query SeoTargets($capability: SeoTargetCapabilityKind) {
  seoTargets(capability: $capability) {
    slug
    displayName
    ownerModuleSlug
    capabilities {
      authoring
      routing
      bulk
      sitemaps
    }
  }
}
`;

export async function fetchSeoTargets(
  options: SeoApiOptions & {
    capability?: SeoTargetCapabilityKind | null;
  } = {}
): Promise<SeoTargetRegistryEntry[]> {
  const variables =
    options.capability === undefined
      ? undefined
      : { capability: options.capability };

  const data = await graphqlRequest<SeoTargetsVariables, SeoTargetsResponse>(
    SEO_TARGETS_QUERY,
    variables,
    options.token,
    options.tenantSlug,
    { graphqlUrl: options.graphqlUrl }
  );

  return data.seoTargets;
}
