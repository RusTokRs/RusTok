import {
  getCacheHealth,
  type GqlOpts,
  type CacheHealthPayload
} from '../api/cache';
import { CacheStatus } from '../components/cache-status';
import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/shadcn/alert';

interface CachePageProps {
  token?: string | null;
  tenantSlug?: string | null;
}

export default async function CachePage({ token, tenantSlug }: CachePageProps) {
  const opts: GqlOpts = { token, tenantSlug };
  let health: CacheHealthPayload | null = null;
  let error: string | null = null;

  try {
    health = await getCacheHealth(opts);
  } catch (err: unknown) {
    error = err instanceof Error ? err.message : 'Failed to load cache health';
  }

  if (error || !health) {
    return (
      <Alert variant='destructive'>
        <AlertTitle>Cache Diagnostics Unavailable</AlertTitle>
        <AlertDescription>
          {error ?? 'Unable to retrieve cache health diagnostics.'}
        </AlertDescription>
      </Alert>
    );
  }

  return <CacheStatus health={health} />;
}
