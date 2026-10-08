import {
  getCacheHealth,
  getCacheSettings,
  DEFAULT_CACHE_SETTINGS,
  type GqlOpts,
  type CacheHealthPayload,
  type CacheSettings
} from '../api/cache';
import { CacheStatus } from '../components/cache-status';
import { CacheSettingsForm } from '../components/cache-settings-form';
import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/shadcn/alert';

interface CachePageProps {
  token?: string | null;
  tenantSlug?: string | null;
}

export default async function CachePage({ token, tenantSlug }: CachePageProps) {
  const opts: GqlOpts = { token, tenantSlug };
  let health: CacheHealthPayload | null = null;
  let settings: CacheSettings = DEFAULT_CACHE_SETTINGS;
  let healthError: string | null = null;

  try {
    const [h, s] = await Promise.all([
      getCacheHealth(opts).catch((err: unknown) => {
        healthError =
          err instanceof Error ? err.message : 'Failed to load cache health';
        return null;
      }),
      getCacheSettings(opts).catch(() => DEFAULT_CACHE_SETTINGS)
    ]);
    health = h;
    settings = s;
  } catch {
    // fallback to defaults
  }

  return (
    <div className='space-y-6'>
      {healthError && !health ? (
        <Alert variant='destructive'>
          <AlertTitle>Cache Diagnostics Unavailable</AlertTitle>
          <AlertDescription>
            {healthError ?? 'Unable to retrieve cache health diagnostics.'}
          </AlertDescription>
        </Alert>
      ) : (
        health && <CacheStatus health={health} />
      )}

      <CacheSettingsForm initialSettings={settings} opts={opts} />
    </div>
  );
}
