import { Badge } from '@/shared/ui/shadcn/badge';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/shared/ui/shadcn/card';
import type { CacheHealthPayload } from '../api/cache';

interface CacheStatusProps {
  health: CacheHealthPayload;
}

export function CacheStatus({ health }: CacheStatusProps) {
  const isHealthy = !health.redisConfigured || health.redisHealthy;

  return (
    <div className='space-y-4'>
      <Card>
        <CardHeader>
          <CardTitle className='flex items-center gap-2'>
            Cache Backend
            <Badge variant={isHealthy ? 'default' : 'destructive'}>
              {isHealthy ? 'Healthy' : 'Unhealthy'}
            </Badge>
          </CardTitle>
          <CardDescription>Current cache infrastructure status</CardDescription>
        </CardHeader>
        <CardContent>
          <dl className='grid grid-cols-2 gap-x-6 gap-y-3 text-sm'>
            <dt className='text-muted-foreground'>Backend</dt>
            <dd className='font-mono font-medium capitalize'>{health.backend}</dd>

            <dt className='text-muted-foreground'>Redis configured</dt>
            <dd>
              <Badge variant={health.redisConfigured ? 'secondary' : 'outline'}>
                {health.redisConfigured ? 'Yes' : 'No'}
              </Badge>
            </dd>

            {health.redisConfigured && (
              <>
                <dt className='text-muted-foreground'>Redis status</dt>
                <dd>
                  <Badge variant={health.redisHealthy ? 'default' : 'destructive'}>
                    {health.redisHealthy ? 'Connected' : 'Disconnected'}
                  </Badge>
                </dd>
              </>
            )}

            {health.redisError && (
              <>
                <dt className='text-muted-foreground'>Error</dt>
                <dd className='font-mono text-destructive break-all'>{health.redisError}</dd>
              </>
            )}
          </dl>
        </CardContent>
      </Card>
    </div>
  );
}
