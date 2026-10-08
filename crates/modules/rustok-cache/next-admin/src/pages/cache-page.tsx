import { getTranslations } from '@rustok/next-fluent/server';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
import {
  getCacheHealth,
  getCacheSettings,
  type GqlOpts
} from '../api/cache';
import { CacheSettingsForm } from '../components/cache-settings-form';

export async function CacheAdminPage(opts: GqlOpts = {}) {
  const t = await getTranslations('cache');
  let health = null;
  let error: string | null = null;
  let settings = null;

  try {
    const [h, s] = await Promise.all([
      getCacheHealth(opts),
      getCacheSettings(opts)
    ]);
    health = h;
    settings = s;
  } catch (e) {
    error = e instanceof Error ? e.message : 'Failed to load cache status';
  }

  return (
    <div className='space-y-6'>
      <div className='grid gap-4 md:grid-cols-2'>
        <Card>
          <CardHeader>
            <CardTitle>{t('health.title')}</CardTitle>
            <CardDescription>{t('health.description')}</CardDescription>
          </CardHeader>
          <CardContent className='space-y-2'>
            {error ? (
              <p className='text-sm text-destructive'>{error}</p>
            ) : health ? (
              <div className='space-y-2 text-sm'>
                <div className='flex justify-between'>
                  <span className='text-muted-foreground'>
                    {t('health.backend')}
                  </span>
                  <span className='font-mono font-medium'>{health.backend}</span>
                </div>
                <div className='flex justify-between'>
                  <span className='text-muted-foreground'>
                    {t('health.configured')}
                  </span>
                  <span
                    className={
                      health.redisConfigured
                        ? 'font-medium text-green-600'
                        : 'font-medium text-muted-foreground'
                    }
                  >
                    {health.redisConfigured ? t('yes') : t('no')}
                  </span>
                </div>
                <div className='flex justify-between'>
                  <span className='text-muted-foreground'>
                    {t('health.healthy')}
                  </span>
                  <span
                    className={
                      health.redisHealthy
                        ? 'font-medium text-green-600'
                        : 'font-medium text-destructive'
                    }
                  >
                    {health.redisHealthy ? t('yes') : t('no')}
                  </span>
                </div>
                {health.redisError && (
                  <div className='pt-2'>
                    <p className='text-xs text-muted-foreground'>
                      {t('health.error')}:
                    </p>
                    <p className='text-xs text-destructive break-all'>
                      {health.redisError}
                    </p>
                  </div>
                )}
              </div>
            ) : null}
          </CardContent>
        </Card>
      </div>

      {settings && (
        <CacheSettingsForm initialSettings={settings} opts={opts} />
      )}
    </div>
  );
}

export const CachePage = CacheAdminPage;

