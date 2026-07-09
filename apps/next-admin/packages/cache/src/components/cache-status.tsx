import { useTranslations } from 'next-intl';
import { Badge } from '@/shared/ui/shadcn/badge';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
import type { CacheHealthPayload } from '../api/cache';

interface CacheStatusProps {
  health: CacheHealthPayload;
}

export function CacheStatus({ health }: CacheStatusProps) {
  const t = useTranslations('cache');
  const isHealthy = !health.redisConfigured || health.redisHealthy;

  return (
    <div className='space-y-4'>
      <Card>
        <CardHeader>
          <CardTitle className='flex items-center gap-2'>
            {t('health.title')}
            <Badge variant={isHealthy ? 'default' : 'destructive'}>
              {isHealthy
                ? t('health.healthyBadge')
                : t('health.unhealthyBadge')}
            </Badge>
          </CardTitle>
          <CardDescription>{t('health.description')}</CardDescription>
        </CardHeader>
        <CardContent>
          <dl className='grid grid-cols-2 gap-x-6 gap-y-3 text-sm'>
            <dt className='text-muted-foreground'>{t('health.backend')}</dt>
            <dd className='font-mono font-medium capitalize'>
              {health.backend}
            </dd>

            <dt className='text-muted-foreground'>{t('health.configured')}</dt>
            <dd>
              <Badge variant={health.redisConfigured ? 'secondary' : 'outline'}>
                {health.redisConfigured ? t('yes') : t('no')}
              </Badge>
            </dd>

            {health.redisConfigured && (
              <>
                <dt className='text-muted-foreground'>{t('health.status')}</dt>
                <dd>
                  <Badge
                    variant={health.redisHealthy ? 'default' : 'destructive'}
                  >
                    {health.redisHealthy
                      ? t('health.connected')
                      : t('health.disconnected')}
                  </Badge>
                </dd>
              </>
            )}

            {health.redisError && (
              <>
                <dt className='text-muted-foreground'>{t('health.error')}</dt>
                <dd className='text-destructive font-mono break-all'>
                  {health.redisError}
                </dd>
              </>
            )}
          </dl>
        </CardContent>
      </Card>
    </div>
  );
}
