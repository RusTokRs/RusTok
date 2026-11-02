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

export interface CacheSettings {
  mode: 'in-memory' | 'redis' | 'hybrid';
  redis_url?: string;
  redis_host?: string;
  redis_port?: number;
  redis_password?: string;
  redis_db?: number;
}

export type CacheSettingsInput = Partial<CacheSettings>;

export const DEFAULT_CACHE_SETTINGS: CacheSettings = {
  mode: 'in-memory',
  redis_url: '',
  redis_host: '127.0.0.1',
  redis_port: 6379,
  redis_password: '',
  redis_db: 0
};

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

const PLATFORM_SETTINGS_QUERY = `
query PlatformSettings($category: String!) {
  platformSettings(category: $category) {
    category
    settings
    schemaVersion
  }
}
`;

const UPDATE_PLATFORM_SETTINGS_MUTATION = `
mutation UpdatePlatformSettings($input: UpdatePlatformSettingsInput!) {
  updatePlatformSettings(input: $input) {
    success
    category
    settings
    schemaVersion
  }
}
`;

interface CacheHealthResponse {
  cacheHealth: CacheHealthPayload;
}

export interface CacheSettingsSnapshot {
  settings: CacheSettings;
  schemaVersion: number;
}

interface PlatformSettingsResponse {
  platformSettings: { category: string; settings: string; schemaVersion: number };
}

interface UpdateSettingsResponse {
  updatePlatformSettings: {
    success: boolean;
    category: string;
    settings: string;
    schemaVersion: number;
  };
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

export async function getCacheSettings(
  opts: GqlOpts = {}
): Promise<CacheSettingsSnapshot> {
  const data = await graphqlRequest<
    { category: string },
    PlatformSettingsResponse
  >(
    PLATFORM_SETTINGS_QUERY,
    { category: 'cache' },
    opts.token,
    opts.tenantSlug
  );

  const schemaVersion = data.platformSettings.schemaVersion;
  try {
    const parsed = JSON.parse(
      data.platformSettings.settings
    ) as Partial<CacheSettings>;
    return {
      settings: {
        ...DEFAULT_CACHE_SETTINGS,
        ...parsed
      },
      schemaVersion
    };
  } catch {
    return { settings: DEFAULT_CACHE_SETTINGS, schemaVersion };
  }
}

export async function updateCacheSettings(
  settings: CacheSettingsInput,
  expectedSchemaVersion: number,
  opts: GqlOpts = {}
): Promise<CacheSettingsSnapshot> {
  const data = await graphqlRequest<
    {
      input: {
        category: string;
        settings: string;
        expectedSchemaVersion: number;
      };
    },
    UpdateSettingsResponse
  >(
    UPDATE_PLATFORM_SETTINGS_MUTATION,
    {
      input: {
        category: 'cache',
        settings: JSON.stringify(settings),
        expectedSchemaVersion
      }
    },
    opts.token,
    opts.tenantSlug
  );

  const schemaVersion = data.updatePlatformSettings.schemaVersion;
  try {
    const parsed = JSON.parse(
      data.updatePlatformSettings.settings
    ) as Partial<CacheSettings>;
    return {
      settings: {
        ...DEFAULT_CACHE_SETTINGS,
        ...parsed
      },
      schemaVersion
    };
  } catch {
    return { settings: DEFAULT_CACHE_SETTINGS, schemaVersion };
  }
}
