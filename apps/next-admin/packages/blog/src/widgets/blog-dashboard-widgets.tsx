'use client';

import React from 'react';
import { StatCard } from '@/widgets/stats-card';
import { IconDatabase } from '@tabler/icons-react';
import type { AdminRenderContext } from '@/modules/types';

export function BlogMetricsWidget({
  context
}: {
  context: AdminRenderContext;
}) {
  return (
    <StatCard
      title='Blog posts'
      value='-'
      helper='published & draft posts'
      icon={<IconDatabase className='size-5' />}
    />
  );
}
