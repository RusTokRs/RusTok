'use client';

import * as React from 'react';
import Link from 'next/link';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Button } from '@/shared/ui/shadcn/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle
} from '@/shared/ui/shadcn/card';
import { DataTableStatic } from '@/widgets/data-table/data-table-static';
import type { DataTableStaticColumn } from '@/widgets/data-table/data-table-static';
import {
  ArrowLeft,
  CircleDot,
  CheckCircle2,
  Clock,
  CreditCard,
  Package,
  Truck,
  XCircle,
  Landmark,
  ReceiptText,
  RotateCcw
} from 'lucide-react';
import type {
  AdminFulfillmentItem,
  AdminOrder,
  AdminOrderDetail,
  AdminOrderLineItem,
  AdminPayment,
  AdminRefund
} from '../../types';

/** Deterministic UTC rendering so SSR and hydration agree on every locale. */
function formatDateTime(value: string | null | undefined): string {
  if (!value) return '—';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  const iso = date.toISOString();
  return `${iso.slice(0, 10)} ${iso.slice(11, 16)} UTC`;
}

function formatAmount(amount: string | null | undefined, currency: string): string {
  if (amount === null || amount === undefined || amount === '') return '—';
  return `${amount} ${currency}`;
}

function shortId(value: string | null | undefined): string {
  if (!value) return '—';
  return value.slice(0, 8);
}

function OrderStatusBadge({ status }: { status: string }) {
  const raw = String(status ?? '').toUpperCase();
  switch (raw) {
    case 'DELIVERED':
      return (
        <Badge
          variant='outline'
          className='border-emerald-500/30 bg-emerald-50 text-emerald-700 capitalize dark:bg-emerald-950/40 dark:text-emerald-300'
        >
          <CheckCircle2 className='mr-1 h-3 w-3' />
          Delivered
        </Badge>
      );
    case 'PAID':
      return (
        <Badge
          variant='outline'
          className='border-indigo-500/30 bg-indigo-50 text-indigo-700 capitalize dark:bg-indigo-950/40 dark:text-indigo-300'
        >
          <CreditCard className='mr-1 h-3 w-3' />
          Paid
        </Badge>
      );
    case 'SHIPPED':
      return (
        <Badge
          variant='outline'
          className='border-sky-500/30 bg-sky-50 text-sky-700 capitalize dark:bg-sky-950/40 dark:text-sky-300'
        >
          <Truck className='mr-1 h-3 w-3' />
          Shipped
        </Badge>
      );
    case 'CONFIRMED':
      return (
        <Badge
          variant='outline'
          className='border-amber-500/30 bg-amber-50 text-amber-700 capitalize dark:bg-amber-950/40 dark:text-amber-300'
        >
          <Clock className='mr-1 h-3 w-3' />
          Confirmed
        </Badge>
      );
    case 'CANCELLED':
      return (
        <Badge
          variant='outline'
          className='border-destructive/30 bg-destructive/10 text-destructive capitalize'
        >
          <XCircle className='mr-1 h-3 w-3' />
          Cancelled
        </Badge>
      );
    default:
      return (
        <Badge variant='secondary' className='capitalize'>
          <CircleDot className='mr-1 h-3 w-3' />
          {raw.toLowerCase()}
        </Badge>
      );
  }
}

function NeutralStatusBadge({ status }: { status: string }) {
  const raw = String(status ?? '');
  return (
    <Badge variant='secondary' className='capitalize'>
      {raw.toLowerCase()}
    </Badge>
  );
}

function InfoRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className='flex flex-col gap-1'>
      <span className='text-muted-foreground text-[11px] font-medium tracking-wide uppercase'>
        {label}
      </span>
      <span className='text-foreground text-sm break-all'>{children}</span>
    </div>
  );
}

const lineItemColumns: DataTableStaticColumn<AdminOrderLineItem>[] = [
  {
    id: 'title',
    header: 'Item',
    cell: (row) => (
      <div className='flex flex-col'>
        <span className='text-foreground text-sm font-medium'>{row.title}</span>
        {row.sku ? (
          <span className='text-muted-foreground font-mono text-[10px]'>{row.sku}</span>
        ) : null}
      </div>
    )
  },
  {
    id: 'quantity',
    header: 'Qty',
    cellClassName: 'w-16 text-center',
    cell: (row) => <span className='text-foreground text-sm'>{row.quantity}</span>
  },
  {
    id: 'unitPrice',
    header: 'Unit price',
    cell: (row) => (
      <span className='text-foreground text-sm'>
        {formatAmount(row.unitPrice, row.currencyCode)}
      </span>
    )
  },
  {
    id: 'totalPrice',
    header: 'Total',
    cell: (row) => (
      <span className='text-foreground text-sm font-semibold'>
        {formatAmount(row.totalPrice, row.currencyCode)}
      </span>
    )
  },
  {
    id: 'fulfillmentRequirement',
    header: 'Fulfillment',
    cell: (row) => (
      <span className='text-muted-foreground text-xs capitalize'>
        {row.fulfillmentRequirement || '—'}
      </span>
    )
  },
  {
    id: 'shippingProfileSlug',
    header: 'Shipping profile',
    cell: (row) => (
      <span className='text-muted-foreground font-mono text-xs'>
        {row.shippingProfileSlug ?? '—'}
      </span>
    )
  }
];

const paymentColumns: DataTableStaticColumn<AdminPayment>[] = [
  {
    id: 'provider',
    header: 'Provider',
    cell: (row) => (
      <div className='flex flex-col'>
        <span className='text-foreground text-sm font-medium'>{row.providerId}</span>
        <span className='text-muted-foreground font-mono text-[10px]'>
          {row.providerPaymentId}
        </span>
      </div>
    )
  },
  {
    id: 'status',
    header: 'Status',
    cell: (row) => <NeutralStatusBadge status={row.status} />
  },
  {
    id: 'amount',
    header: 'Amount',
    cell: (row) => (
      <span className='text-foreground text-sm'>
        {formatAmount(row.amount, row.currencyCode)}
      </span>
    )
  },
  {
    id: 'capturedAmount',
    header: 'Captured',
    cell: (row) => (
      <span className='text-foreground text-sm'>
        {formatAmount(row.capturedAmount, row.currencyCode)}
      </span>
    )
  },
  {
    id: 'capturedAt',
    header: 'Captured at',
    cell: (row) => (
      <span className='text-muted-foreground text-xs'>
        {formatDateTime(row.capturedAt ?? row.authorizedAt ?? row.createdAt)}
      </span>
    )
  },
  {
    id: 'errorMessage',
    header: 'Error',
    cell: (row) =>
      row.errorMessage ? (
        <span className='text-destructive text-xs'>{row.errorMessage}</span>
      ) : (
        <span className='text-muted-foreground text-xs'>—</span>
      )
  }
];

const refundColumns: DataTableStaticColumn<AdminRefund>[] = [
  {
    id: 'id',
    header: 'Refund',
    cell: (row) => (
      <span className='text-foreground font-mono text-xs'>{shortId(row.id)}</span>
    )
  },
  {
    id: 'status',
    header: 'Status',
    cell: (row) => <NeutralStatusBadge status={row.status} />
  },
  {
    id: 'amount',
    header: 'Amount',
    cell: (row) => (
      <span className='text-foreground text-sm font-semibold'>
        {formatAmount(row.amount, row.currencyCode)}
      </span>
    )
  },
  {
    id: 'reason',
    header: 'Reason',
    cell: (row) => (
      <span className='text-muted-foreground text-xs'>{row.reason ?? '—'}</span>
    )
  },
  {
    id: 'refundedAt',
    header: 'Refunded at',
    cell: (row) => (
      <span className='text-muted-foreground text-xs'>
        {formatDateTime(row.refundedAt ?? row.createdAt)}
      </span>
    )
  }
];

const fulfillmentItemColumns: DataTableStaticColumn<AdminFulfillmentItem>[] = [
  {
    id: 'orderLineItemId',
    header: 'Line item',
    cell: (row) => (
      <span className='text-foreground font-mono text-xs'>
        {shortId(row.orderLineItemId)}
      </span>
    )
  },
  {
    id: 'quantity',
    header: 'Ordered',
    cellClassName: 'w-20 text-center',
    cell: (row) => <span className='text-foreground text-sm'>{row.quantity}</span>
  },
  {
    id: 'shippedQuantity',
    header: 'Shipped',
    cellClassName: 'w-20 text-center',
    cell: (row) => <span className='text-foreground text-sm'>{row.shippedQuantity}</span>
  },
  {
    id: 'deliveredQuantity',
    header: 'Delivered',
    cellClassName: 'w-20 text-center',
    cell: (row) => <span className='text-foreground text-sm'>{row.deliveredQuantity}</span>
  }
];

function OrderSummaryCard({ order }: { order: AdminOrder }) {
  const timeline: Array<{ label: string; value: string | null }> = [
    { label: 'Created', value: order.createdAt },
    { label: 'Confirmed', value: order.confirmedAt },
    { label: 'Paid', value: order.paidAt },
    { label: 'Shipped', value: order.shippedAt },
    { label: 'Delivered', value: order.deliveredAt },
    { label: 'Cancelled', value: order.cancelledAt }
  ];

  const amounts: Array<{ label: string; value: string; strong?: boolean }> = [
    { label: 'Subtotal', value: formatAmount(order.subtotalAmount, order.currencyCode) },
    {
      label: 'Shipping',
      value: formatAmount(order.shippingTotal, order.currencyCode)
    },
    {
      label: 'Adjustments',
      value: formatAmount(order.adjustmentTotal, order.currencyCode)
    },
    {
      label: order.taxIncluded ? 'Tax (included)' : 'Tax',
      value: formatAmount(order.taxTotal, order.currencyCode)
    },
    {
      label: 'Total',
      value: formatAmount(order.totalAmount, order.currencyCode),
      strong: true
    }
  ];

  return (
    <div className='grid gap-4 lg:grid-cols-2'>
      <Card>
        <CardHeader>
          <CardTitle className='text-base'>Summary</CardTitle>
          <CardDescription>Customer, channel and order amounts.</CardDescription>
        </CardHeader>
        <CardContent className='flex flex-col gap-6'>
          <div className='grid grid-cols-2 gap-4'>
            <InfoRow label='Customer'>
              <span className='font-mono text-xs'>{shortId(order.customerId)}</span>
            </InfoRow>
            <InfoRow label='Channel'>
              {order.channelSlug ?? shortId(order.channelId)}
            </InfoRow>
            <InfoRow label='Payment method'>{order.paymentMethod ?? '—'}</InfoRow>
            <InfoRow label='Carrier'>{order.carrier ?? '—'}</InfoRow>
            <InfoRow label='Tracking number'>
              <span className='font-mono text-xs'>{order.trackingNumber ?? '—'}</span>
            </InfoRow>
            <InfoRow label='Order ID'>
              <span className='font-mono text-xs'>{order.id}</span>
            </InfoRow>
          </div>
          <div className='flex flex-col gap-2'>
            {amounts.map((entry) => (
              <div
                key={entry.label}
                className='flex items-center justify-between gap-4'
              >
                <span
                  className={
                    entry.strong
                      ? 'text-foreground text-sm font-semibold'
                      : 'text-muted-foreground text-sm'
                  }
                >
                  {entry.label}
                </span>
                <span
                  className={
                    entry.strong
                      ? 'text-foreground text-sm font-semibold'
                      : 'text-foreground text-sm'
                  }
                >
                  {entry.value}
                </span>
              </div>
            ))}
          </div>
          {order.cancellationReason ? (
            <div className='border-destructive/30 bg-destructive/5 rounded-md border p-3'>
              <p className='text-destructive text-xs font-semibold'>
                Cancellation reason
              </p>
              <p className='text-destructive/90 text-sm'>
                {order.cancellationReason}
              </p>
            </div>
          ) : null}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className='text-base'>Timeline</CardTitle>
          <CardDescription>Order lifecycle milestones (UTC).</CardDescription>
        </CardHeader>
        <CardContent className='flex flex-col gap-3'>
          {timeline.map((entry) => (
            <div
              key={entry.label}
              className='flex items-center justify-between gap-4'
            >
              <span className='text-muted-foreground text-sm'>{entry.label}</span>
              <span
                className={
                  entry.value
                    ? 'text-foreground text-sm'
                    : 'text-muted-foreground/60 text-sm'
                }
              >
                {formatDateTime(entry.value)}
              </span>
            </div>
          ))}
          {order.deliveredSignature ? (
            <div className='rounded-md border p-3'>
              <p className='text-muted-foreground text-[11px] font-medium tracking-wide uppercase'>
                Delivered signature
              </p>
              <p className='text-foreground text-sm break-all'>
                {order.deliveredSignature}
              </p>
            </div>
          ) : null}
        </CardContent>
      </Card>
    </div>
  );
}

export function OrderDetailView({ detail }: { detail: AdminOrderDetail }) {
  const { order, paymentCollection, fulfillment } = detail;

  return (
    <div className='flex flex-1 flex-col gap-4'>
      <div className='flex flex-wrap items-center justify-between gap-3'>
        <div className='flex items-center gap-3'>
          <Button asChild variant='ghost' size='icon' className='h-8 w-8 p-0'>
            <Link href='/dashboard/commerce/orders' aria-label='Back to orders'>
              <ArrowLeft className='h-4 w-4' />
            </Link>
          </Button>
          <h1 className='text-lg font-semibold'>Order #{shortId(order.id)}</h1>
          <OrderStatusBadge status={order.status} />
        </div>
        <span className='text-muted-foreground text-xs'>
          Updated {formatDateTime(order.updatedAt)}
        </span>
      </div>

      <OrderSummaryCard order={order} />

      <Card>
        <CardHeader>
          <CardTitle className='flex items-center gap-2 text-base'>
            <Package className='size-4' />
            Line items
          </CardTitle>
          <CardDescription>
            {order.lineItems.length}{' '}
            {order.lineItems.length === 1 ? 'item' : 'items'} in this order.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <DataTableStatic
            rows={order.lineItems}
            columns={lineItemColumns}
            getRowKey={(row) => row.id}
            emptyState='This order has no line items.'
            ariaLabel='Order line items'
          />
        </CardContent>
      </Card>

      {order.adjustments.length > 0 ? (
        <Card>
          <CardHeader>
            <CardTitle className='flex items-center gap-2 text-base'>
              <ReceiptText className='size-4' />
              Adjustments
            </CardTitle>
            <CardDescription>Applied order-level adjustments.</CardDescription>
          </CardHeader>
          <CardContent className='flex flex-col gap-2'>
            {order.adjustments.map((adjustment) => (
              <div
                key={adjustment.id}
                className='flex items-center justify-between gap-4 rounded-md border p-3'
              >
                <div className='flex flex-col'>
                  <span className='text-foreground text-sm font-medium'>
                    {adjustment.sourceType}
                  </span>
                  {adjustment.sourceId ? (
                    <span className='text-muted-foreground font-mono text-[10px]'>
                      {adjustment.sourceId}
                    </span>
                  ) : null}
                </div>
                <span className='text-foreground text-sm font-semibold'>
                  {formatAmount(adjustment.amount, adjustment.currencyCode)}
                </span>
              </div>
            ))}
          </CardContent>
        </Card>
      ) : null}

      {order.taxLines.length > 0 ? (
        <Card>
          <CardHeader>
            <CardTitle className='flex items-center gap-2 text-base'>
              <Landmark className='size-4' />
              Tax lines
            </CardTitle>
            <CardDescription>
              Tax computed per line by the tax provider.
            </CardDescription>
          </CardHeader>
          <CardContent className='flex flex-col gap-2'>
            {order.taxLines.map((taxLine) => (
              <div
                key={taxLine.id}
                className='flex items-center justify-between gap-4 rounded-md border p-3'
              >
                <div className='flex flex-col'>
                  <span className='text-foreground text-sm font-medium'>
                    {taxLine.description ?? taxLine.providerId}
                  </span>
                  <span className='text-muted-foreground font-mono text-[10px]'>
                    rate {taxLine.rate}
                  </span>
                </div>
                <span className='text-foreground text-sm font-semibold'>
                  {formatAmount(taxLine.amount, taxLine.currencyCode)}
                </span>
              </div>
            ))}
          </CardContent>
        </Card>
      ) : null}

      <Card>
        <CardHeader>
          <CardTitle className='flex items-center gap-2 text-base'>
            <CreditCard className='size-4' />
            Payment
          </CardTitle>
          <CardDescription>
            Latest payment collection with provider payments and refunds.
          </CardDescription>
        </CardHeader>
        <CardContent className='flex flex-col gap-4'>
          {paymentCollection ? (
            <>
              <div className='grid grid-cols-2 gap-4 md:grid-cols-4'>
                <InfoRow label='Status'>
                  <NeutralStatusBadge status={paymentCollection.status} />
                </InfoRow>
                <InfoRow label='Provider'>
                  {paymentCollection.providerId ?? '—'}
                </InfoRow>
                <InfoRow label='Collection'>
                  <span className='font-mono text-xs'>
                    {shortId(paymentCollection.id)}
                  </span>
                </InfoRow>
                <InfoRow label='Authorized / captured'>
                  {formatAmount(paymentCollection.authorizedAmount, paymentCollection.currencyCode)}
                  {' / '}
                  {formatAmount(paymentCollection.capturedAmount, paymentCollection.currencyCode)}
                </InfoRow>
                <InfoRow label='Collected'>
                  {formatAmount(paymentCollection.amount, paymentCollection.currencyCode)}
                </InfoRow>
                <InfoRow label='Refunded'>
                  {formatAmount(paymentCollection.refundedAmount, paymentCollection.currencyCode)}
                </InfoRow>
                <InfoRow label='Created'>
                  {formatDateTime(paymentCollection.createdAt)}
                </InfoRow>
                <InfoRow label='Cancelled'>
                  {formatDateTime(paymentCollection.cancelledAt)}
                </InfoRow>
              </div>
              <DataTableStatic
                rows={paymentCollection.payments}
                columns={paymentColumns}
                getRowKey={(row) => row.id}
                emptyState='No provider payments recorded for this collection.'
                ariaLabel='Provider payments'
              />
              <DataTableStatic
                rows={paymentCollection.refunds}
                columns={refundColumns}
                getRowKey={(row) => row.id}
                emptyState='No refunds recorded for this collection.'
                ariaLabel='Refunds'
              />
            </>
          ) : (
            <p className='text-muted-foreground text-sm'>
              No payment collection is attached to this order.
            </p>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className='flex items-center gap-2 text-base'>
            <Truck className='size-4' />
            Fulfillment
          </CardTitle>
          <CardDescription>
            Latest fulfillment shipment and per-line delivery progress.
          </CardDescription>
        </CardHeader>
        <CardContent className='flex flex-col gap-4'>
          {fulfillment ? (
            <>
              <div className='grid grid-cols-2 gap-4 md:grid-cols-4'>
                <InfoRow label='Status'>
                  <NeutralStatusBadge status={fulfillment.status} />
                </InfoRow>
                <InfoRow label='Carrier'>{fulfillment.carrier ?? '—'}</InfoRow>
                <InfoRow label='Tracking number'>
                  <span className='font-mono text-xs'>
                    {fulfillment.trackingNumber ?? '—'}
                  </span>
                </InfoRow>
                <InfoRow label='Shipped'>
                  {formatDateTime(fulfillment.shippedAt)}
                </InfoRow>
                <InfoRow label='Delivered'>
                  {formatDateTime(fulfillment.deliveredAt)}
                </InfoRow>
                <InfoRow label='Cancelled'>
                  {formatDateTime(fulfillment.cancelledAt)}
                </InfoRow>
              </div>
              {fulfillment.cancellationReason ? (
                <div className='border-destructive/30 bg-destructive/5 rounded-md border p-3'>
                  <p className='text-destructive text-xs font-semibold'>
                    Fulfillment cancellation reason
                  </p>
                  <p className='text-destructive/90 text-sm'>
                    {fulfillment.cancellationReason}
                  </p>
                </div>
              ) : null}
              {fulfillment.deliveredNote ? (
                <div className='rounded-md border p-3'>
                  <p className='text-muted-foreground text-[11px] font-medium tracking-wide uppercase'>
                    Delivery note
                  </p>
                  <p className='text-foreground text-sm'>
                    {fulfillment.deliveredNote}
                  </p>
                </div>
              ) : null}
              <DataTableStatic
                rows={fulfillment.items}
                columns={fulfillmentItemColumns}
                getRowKey={(row) => row.id}
                emptyState='No fulfillment lines recorded for this shipment.'
                ariaLabel='Fulfillment lines'
              />
            </>
          ) : (
            <p className='text-muted-foreground text-sm'>
              This order has not been fulfilled yet.
            </p>
          )}
        </CardContent>
      </Card>

      <p className='text-muted-foreground/60 flex items-center gap-1.5 text-xs'>
        <RotateCcw className='size-3' />
        Reload the page to refresh payment and fulfillment snapshots.
      </p>
    </div>
  );
}
