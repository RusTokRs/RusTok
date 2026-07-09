import { graphqlRequest } from '@/lib/graphql';

export interface GqlOpts {
  token?: string | null;
  tenantSlug?: string | null;
}

export interface CacheHealthPayload {
  redisConfigured: boolean;
  redisHealthy: boolean;
  redisError: string | null;
  backend: string;
}

const CACHE_HEALTH_QUERY = `
query CacheHealth {
  cacheHealth {
    redisConfigured
    redisHealthy
    redisError
    backend
  }
}
`;

interface CacheHealthResponse {
  cacheHealth: CacheHealthPayload;
}

export async function getCacheHealth(
  opts: GqlOpts = {}
): Promise<CacheHealthPayload> {
  const data = await graphqlRequest<Record<string, never>, CacheHealthResponse>(
    CACHE_HEALTH_QUERY,
    {},
    opts.token,
    opts.tenantSlug
  );
  return data.cacheHealth;
}
