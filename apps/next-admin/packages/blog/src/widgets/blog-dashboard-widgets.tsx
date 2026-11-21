'use client';

import React from 'react';
import { StatCard } from '@/widgets/stats-card';
import { IconDatabase } from '@tabler/icons-react';
import type { AdminRenderContext } from '@/modules/types';
import { fetchBlogStats } from '../api/stats';
import type { BlogStats } from '../api/stats';

export function BlogMetricsWidget({
  context
}: {
  context: AdminRenderContext;
}) {
  const [stats, setStats] = React.useState<BlogStats | null>(null);
  const [failed, setFailed] = React.useState(false);

  const token = context.token;
  const tenantSlug = context.tenantSlug;
  const tenantId = context.tenantId;

  React.useEffect(() => {
    if (!token || !tenantSlug || !tenantId) {
      setFailed(true);
      return;
    }

    let cancelled = false;
    void (async () => {
      try {
        const result = await fetchBlogStats({ token, tenantSlug, tenantId });
        if (!cancelled) setStats(result);
      } catch {
        if (!cancelled) setFailed(true);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [token, tenantSlug, tenantId]);

  const value = stats
    ? stats.total.toLocaleString()
    : failed
      ? '—'
      : '…';
  const helper = stats
    ? `published ${stats.published.toLocaleString()} · draft ${stats.draft.toLocaleString()}`
    : failed
      ? 'unavailable — check sign-in'
      : 'loading counters';

  return (
    <StatCard
      title='Blog posts'
      value={value}
      helper={helper}
      icon={<IconDatabase className='size-5' />}
    />
  );
}
