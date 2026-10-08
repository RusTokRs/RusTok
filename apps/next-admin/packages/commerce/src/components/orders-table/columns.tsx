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
  Text,
  CreditCard
} from 'lucide-react';
import Link from 'next/link';
import type { OrderListItem } from '../../types';
import { CellAction } from './cell-action';

export const STATUS_OPTIONS = [
  { label: 'Draft', value: 'DRAFT' },
  { label: 'Confirmed', value: 'CONFIRMED' },
  { label: 'Paid', value: 'PAID' },
  { label: 'Shipped', value: 'SHIPPED' },
  { label: 'Delivered', value: 'DELIVERED' },
  { label: 'Cancelled', value: 'CANCELLED' }
];

export const columns: ColumnDef<StockFeatures, OrderListItem, any>[] = [
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
      <DataTableColumnHeader column={column} title='Order' />
    ),
    cell: ({ row }) => {
      const order = row.original;
      const shortId = order.id.slice(0, 8);
      return (
        <div className='flex flex-col'>
          <Link
            href={`/dashboard/commerce/orders/${order.id}`}
            className='hover:text-primary text-foreground max-w-[200px] truncate font-mono text-xs font-semibold transition hover:underline'
            title={order.id}
          >
            #{shortId}
          </Link>
          <span className='text-muted-foreground font-mono text-[10px]'>
            {order.id}
          </span>
        </div>
      );
    },
    meta: {
      label: 'Order ID',
      placeholder: 'Filter by ID...',
      variant: 'text',
      icon: Text
    },
    enableColumnFilter: true
  },
  {
    id: 'customerId',
    accessorKey: 'customerId',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Customer' />
    ),
    cell: ({ getValue }) => {
      const val = getValue() as string;
      const short = val ? val.slice(0, 8) : '—';
      return (
        <span className='text-muted-foreground font-mono text-xs' title={val}>
          {short}
        </span>
      );
    },
    meta: {
      label: 'Customer ID',
      placeholder: 'Filter customer...',
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
      const raw = String(getValue() ?? '').toUpperCase();
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
    },
    enableColumnFilter: true,
    meta: {
      label: 'Status',
      variant: 'multiSelect',
      options: STATUS_OPTIONS
    }
  },
  {
    id: 'items',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Items' />
    ),
    cell: ({ row }) => {
      const count = row.original.lineItems?.length ?? 0;
      return (
        <div className='text-muted-foreground flex items-center gap-1.5 text-xs'>
          <Package className='h-3.5 w-3.5' />
          <span>
            {count} {count === 1 ? 'item' : 'items'}
          </span>
        </div>
      );
    }
  },
  {
    id: 'totalAmount',
    accessorKey: 'totalAmount',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Total' />
    ),
    cell: ({ row }) => {
      const order = row.original;
      return (
        <span className='text-foreground text-xs font-semibold tracking-tight'>
          {order.totalAmount} {order.currencyCode}
        </span>
      );
    }
  },
  {
    id: 'carrier',
    accessorKey: 'carrier',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Tracking' />
    ),
    cell: ({ row }) => {
      const order = row.original;
      if (!order.carrier && !order.trackingNumber) {
        return <span className='text-muted-foreground text-xs'>—</span>;
      }
      return (
        <div className='flex flex-col text-xs'>
          {order.carrier && (
            <span className='text-foreground font-medium'>{order.carrier}</span>
          )}
          {order.trackingNumber && (
            <span className='text-muted-foreground font-mono text-[10px]'>
              {order.trackingNumber}
            </span>
          )}
        </div>
      );
    }
  },
  {
    id: 'createdAt',
    accessorKey: 'createdAt',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Created' />
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
