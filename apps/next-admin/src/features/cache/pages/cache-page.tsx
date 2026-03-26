import { getCacheHealth, type GqlOpts } from '../api/cache';
import { CacheStatus } from '../components/cache-status';

interface CachePageProps {
  token?: string | null;
  tenantSlug?: string | null;
}

export default async function CachePage({ token, tenantSlug }: CachePageProps) {
  const opts: GqlOpts = { token, tenantSlug };
  const health = await getCacheHealth(opts);

  return <CacheStatus health={health} />;
}
