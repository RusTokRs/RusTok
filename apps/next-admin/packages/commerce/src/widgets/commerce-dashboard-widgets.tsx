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
import { Badge } from '@/shared/ui/shadcn/badge';
import {
  IconReceipt,
  IconTruck,
  IconArrowUpRight
} from '@tabler/icons-react';
import type { AdminRenderContext } from '@/modules/types';
import { listFulfillments, listOrders } from '../api';
import type { OrderListItem } from '../types';

interface CommerceMetricsState {
  ordersTotal: number | null;
  pendingFulfillments: number | null;
  failed: boolean;
}

const METRICS_INITIAL: CommerceMetricsState = {
  ordersTotal: null,
  pendingFulfillments: null,
  failed: false
};

function useCommerceMetrics(context: AdminRenderContext): CommerceMetricsState {
  const [state, setState] = React.useState<CommerceMetricsState>(METRICS_INITIAL);

  const token = context.token;
  const tenantSlug = context.tenantSlug;
  const tenantId = context.tenantId;

  React.useEffect(() => {
    if (!token || !tenantSlug || !tenantId) {
      setState({ ...METRICS_INITIAL, failed: true });
      return;
    }

    let cancelled = false;
    const opts = { token, tenantSlug, tenantId };

    void (async () => {
      try {
        const [orders, pending] = await Promise.all([
          listOrders(opts, { page: 1, perPage: 1 }),
          listFulfillments(opts, { page: 1, perPage: 1, status: 'pending' })
        ]);
        if (cancelled) return;
        setState({
          ordersTotal: orders.total,
          pendingFulfillments: pending.total,
          failed: false
        });
      } catch {
        if (!cancelled) {
          setState({ ...METRICS_INITIAL, failed: true });
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [token, tenantSlug, tenantId]);

  return state;
}

export function CommerceMetricsWidget({
  context
}: {
  context: AdminRenderContext;
}) {
  const { ordersTotal, pendingFulfillments, failed } = useCommerceMetrics(context);

  const pendingValue =
    pendingFulfillments === null ? (failed ? '—' : '…') : pendingFulfillments.toLocaleString();
  const ordersValue =
    ordersTotal === null ? (failed ? '—' : '…') : ordersTotal.toLocaleString();

  return (
    <>
      <StatCard
        title='Orders'
        value={ordersValue}
        helper={failed ? 'unavailable — check sign-in' : 'across all statuses'}
        icon={<IconReceipt className='size-5' />}
      />
      <StatCard
        title='Pending fulfillments'
        value={pendingValue}
        helper={failed ? 'unavailable — check sign-in' : 'awaiting shipment'}
        icon={<IconTruck className='size-5' />}
      />
    </>
  );
}

function RecentOrderStatusBadge({ status }: { status: string }) {
  const raw = String(status ?? '').toUpperCase();
  switch (raw) {
    case 'DELIVERED':
      return (
        <Badge
          variant='outline'
          className='border-emerald-500/30 bg-emerald-50 text-emerald-700 dark:bg-emerald-950/40 dark:text-emerald-300'
        >
          Delivered
        </Badge>
      );
    case 'PAID':
      return (
        <Badge
          variant='outline'
          className='border-indigo-500/30 bg-indigo-50 text-indigo-700 dark:bg-indigo-950/40 dark:text-indigo-300'
        >
          Paid
        </Badge>
      );
    case 'SHIPPED':
      return (
        <Badge
          variant='outline'
          className='border-sky-500/30 bg-sky-50 text-sky-700 dark:bg-sky-950/40 dark:text-sky-300'
        >
          Shipped
        </Badge>
      );
    case 'CANCELLED':
      return (
        <Badge
          variant='outline'
          className='border-destructive/30 bg-destructive/10 text-destructive'
        >
          Cancelled
        </Badge>
      );
    default:
      return (
        <Badge variant='secondary' className='capitalize'>
          {raw.toLowerCase()}
        </Badge>
      );
  }
}

export function CommerceRecentOrdersWidget({
  context
}: {
  context: AdminRenderContext;
}) {
  const [orders, setOrders] = React.useState<OrderListItem[] | null>(null);
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
        const result = await listOrders(
          { token, tenantSlug, tenantId },
          { page: 1, perPage: 5 }
        );
        if (!cancelled) setOrders(result.items);
      } catch {
        if (!cancelled) setFailed(true);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [token, tenantSlug, tenantId]);

  return (
    <Card>
      <CardHeader>
        <CardTitle>Commerce overview</CardTitle>
        <CardDescription>Latest orders across every status.</CardDescription>
      </CardHeader>
      <CardContent className='flex flex-col gap-2'>
        {failed ? (
          <p className='text-muted-foreground text-sm'>
            Recent orders are unavailable. Reconnect and reload the dashboard.
          </p>
        ) : orders === null ? (
          <p className='text-muted-foreground text-sm'>Loading recent orders…</p>
        ) : orders.length === 0 ? (
          <p className='text-muted-foreground text-sm'>
            No orders yet. Orders appear here as soon as checkout completes.
          </p>
        ) : (
          orders.map((order) => (
            <Link
              key={order.id}
              href={`/dashboard/commerce/orders/${order.id}`}
              className='hover:bg-accent flex items-center justify-between gap-3 rounded-md border p-2.5 transition'
            >
              <div className='flex min-w-0 flex-col gap-0.5'>
                <span className='text-foreground truncate font-mono text-xs font-semibold'>
                  #{order.id.slice(0, 8)}
                </span>
                <span className='text-muted-foreground text-[11px]'>
                  {order.lineItems?.length ?? 0}{' '}
                  {(order.lineItems?.length ?? 0) === 1 ? 'item' : 'items'} ·{' '}
                  {order.totalAmount} {order.currencyCode}
                </span>
              </div>
              <RecentOrderStatusBadge status={order.status} />
            </Link>
          ))
        )}
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
