'use client';

import * as React from 'react';
import type { ColumnDef, StockFeatures } from '@tanstack/react-table';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Checkbox } from '@/shared/ui/shadcn/checkbox';
import { DataTableColumnHeader } from '@/widgets/data-table/data-table-column-header';
import {
  CircleDot,
  CheckCircle2,
  Clock,
  Truck,
  XCircle,
  Package,
  Text
} from 'lucide-react';
import Link from 'next/link';
import type { AdminFulfillment } from '../../types';
import { CellAction } from './cell-action';

export const FULFILLMENT_STATUS_OPTIONS = [
  { label: 'Pending', value: 'pending' },
  { label: 'Shipped', value: 'shipped' },
  { label: 'Delivered', value: 'delivered' },
  { label: 'Cancelled', value: 'cancelled' }
];

export const columns: ColumnDef<StockFeatures, AdminFulfillment, any>[] = [
  {
    id: 'select',
    header: ({ table }) => (
      <Checkbox
        checked={
          table.getIsAllPageRowsSelected() ||
          (table.getIsSomePageRowsSelected() && 'indeterminate')
        }
        onCheckedChange={(value) => table.toggleAllPageRowsSelected(!!value)}
        aria-label='Select all'
        className='translate-y-0.5'
      />
    ),
    cell: ({ row }) => (
      <Checkbox
        checked={row.getIsSelected()}
        onCheckedChange={(value) => row.toggleSelected(!!value)}
        aria-label='Select row'
        className='translate-y-0.5'
      />
    ),
    enableSorting: false,
    enableHiding: false,
    size: 40
  },
  {
    id: 'id',
    accessorKey: 'id',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Fulfillment' />
    ),
    cell: ({ row }) => {
      const fulfillment = row.original;
      const shortId = fulfillment.id.slice(0, 8);
      return (
        <div className='flex flex-col'>
          <span className='text-foreground max-w-[200px] truncate font-mono text-xs font-semibold'>
            #{shortId}
          </span>
          <span className='text-muted-foreground font-mono text-[10px]'>
            {fulfillment.id}
          </span>
        </div>
      );
    },
    meta: {
      label: 'Fulfillment ID',
      placeholder: 'Filter by ID...',
      variant: 'text',
      icon: Text
    },
    enableColumnFilter: true
  },
  {
    id: 'orderId',
    accessorKey: 'orderId',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Order' />
    ),
    cell: ({ row }) => {
      const val = row.original.orderId;
      return (
        <Link
          href={`/dashboard/commerce/orders/${val}`}
          className='hover:text-primary text-foreground font-mono text-xs transition hover:underline'
          title={val}
        >
          #{val.slice(0, 8)}
        </Link>
      );
    },
    meta: {
      label: 'Order ID',
      placeholder: 'Filter order...',
      variant: 'text'
    },
    enableColumnFilter: true
  },
  {
    id: 'status',
    accessorKey: 'status',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Status' />
    ),
    cell: ({ getValue }) => {
      const raw = String(getValue() ?? '').toLowerCase();
      switch (raw) {
        case 'delivered':
          return (
            <Badge
              variant='outline'
              className='border-emerald-500/30 bg-emerald-50 text-emerald-700 capitalize dark:bg-emerald-950/40 dark:text-emerald-300'
            >
              <CheckCircle2 className='mr-1 h-3 w-3' />
              Delivered
            </Badge>
          );
        case 'shipped':
          return (
            <Badge
              variant='outline'
              className='border-sky-500/30 bg-sky-50 text-sky-700 capitalize dark:bg-sky-950/40 dark:text-sky-300'
            >
              <Truck className='mr-1 h-3 w-3' />
              Shipped
            </Badge>
          );
        case 'pending':
          return (
            <Badge
              variant='outline'
              className='border-amber-500/30 bg-amber-50 text-amber-700 capitalize dark:bg-amber-950/40 dark:text-amber-300'
            >
              <Clock className='mr-1 h-3 w-3' />
              Pending
            </Badge>
          );
        case 'cancelled':
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
              {raw}
            </Badge>
          );
      }
    },
    enableColumnFilter: true,
    meta: {
      label: 'Status',
      variant: 'multiSelect',
      options: FULFILLMENT_STATUS_OPTIONS
    }
  },
  {
    id: 'carrier',
    accessorKey: 'carrier',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Tracking' />
    ),
    cell: ({ row }) => {
      const fulfillment = row.original;
      if (!fulfillment.carrier && !fulfillment.trackingNumber) {
        return <span className='text-muted-foreground text-xs'>—</span>;
      }
      return (
        <div className='flex flex-col text-xs'>
          {fulfillment.carrier ? (
            <span className='text-foreground font-medium'>
              {fulfillment.carrier}
            </span>
          ) : null}
          {fulfillment.trackingNumber ? (
            <span className='text-muted-foreground font-mono text-[10px]'>
              {fulfillment.trackingNumber}
            </span>
          ) : null}
        </div>
      );
    }
  },
  {
    id: 'items',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Lines' />
    ),
    cell: ({ row }) => {
      const count = row.original.items?.length ?? 0;
      const total = row.original.items?.reduce(
        (sum, item) => sum + item.quantity,
        0
      );
      return (
        <div className='text-muted-foreground flex items-center gap-1.5 text-xs'>
          <Package className='h-3.5 w-3.5' />
          <span>
            {count} {count === 1 ? 'line' : 'lines'}
            {total ? ` · ${total} pcs` : ''}
          </span>
        </div>
      );
    }
  },
  {
    id: 'shippedAt',
    accessorKey: 'shippedAt',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Shipped' />
    ),
    cell: ({ getValue }) => {
      const raw = getValue() as string | null;
      if (!raw) return <span className='text-muted-foreground text-xs'>—</span>;
      return (
        <span className='text-muted-foreground text-xs whitespace-nowrap'>
          {new Date(raw).toLocaleDateString()}
        </span>
      );
    }
  },
  {
    id: 'deliveredAt',
    accessorKey: 'deliveredAt',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Delivered' />
    ),
    cell: ({ getValue }) => {
      const raw = getValue() as string | null;
      if (!raw) return <span className='text-muted-foreground text-xs'>—</span>;
      return (
        <span className='text-muted-foreground text-xs whitespace-nowrap'>
          {new Date(raw).toLocaleDateString()}
        </span>
      );
    }
  },
  {
    id: 'actions',
    cell: ({ row }) => <CellAction data={row.original} />,
    size: 50
  }
];
