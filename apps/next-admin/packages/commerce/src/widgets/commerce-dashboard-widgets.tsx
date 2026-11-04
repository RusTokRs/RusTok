'use client';

import React from 'react';
import Link from 'next/link';
import { StatCard } from '@/widgets/stats-card';
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter
} from '@/shared/ui/shadcn/card';
import { Button } from '@/shared/ui/shadcn/button';
import { IconReceipt, IconActivity, IconArrowUpRight } from '@tabler/icons-react';
import type { AdminRenderContext } from '@/modules/types';

export function CommerceMetricsWidget({ context }: { context: AdminRenderContext }) {
  // In an ideal FFA module, module metrics can be hydrated from context or module query.
  return (
    <>
      <StatCard
        title='Orders'
        value='-'
        helper='from commerce orders'
        icon={<IconReceipt className='size-5' />}
      />
      <StatCard
        title='Revenue snapshot'
        value='-'
        helper='from completed orders'
        icon={<IconActivity className='size-5' />}
      />
    </>
  );
}

export function CommerceRecentOrdersWidget({ context }: { context: AdminRenderContext }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Commerce overview</CardTitle>
        <CardDescription>
          Order fulfillment and promotion statuses.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <p className='text-muted-foreground text-sm'>
          Manage pending orders, shipping profiles, and promotions directly in Commerce management.
        </p>
      </CardContent>
      <CardFooter>
        <Button asChild variant='outline' className='w-full'>
          <Link href='/dashboard/commerce/orders'>
            Open orders
            <IconArrowUpRight className='ml-2 size-4' />
          </Link>
        </Button>
      </CardFooter>
    </Card>
  );
}

