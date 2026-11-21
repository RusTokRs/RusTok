'use client';

import * as React from 'react';
import type { ColumnDef, StockFeatures } from '@tanstack/react-table';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Checkbox } from '@/shared/ui/shadcn/checkbox';
import { DataTableColumnHeader } from '@/widgets/data-table/data-table-column-header';
import {
  CircleDot,
  Clock,
  XCircle,
  CreditCard,
  ShieldCheck,
  Text
} from 'lucide-react';
import Link from 'next/link';
import type { AdminPaymentCollection } from '../../types';
import { CellAction } from './cell-action';

export const PAYMENT_STATUS_OPTIONS = [
  { label: 'Pending', value: 'pending' },
  { label: 'Authorized', value: 'authorized' },
  { label: 'Captured', value: 'captured' },
  { label: 'Cancelled', value: 'cancelled' }
];

export const columns: ColumnDef<StockFeatures, AdminPaymentCollection, any>[] = [
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
      <DataTableColumnHeader column={column} title='Collection' />
    ),
    cell: ({ row }) => {
      const collection = row.original;
      const shortId = collection.id.slice(0, 8);
      return (
        <div className='flex flex-col'>
          <span className='text-foreground max-w-[200px] truncate font-mono text-xs font-semibold'>
            #{shortId}
          </span>
          <span className='text-muted-foreground font-mono text-[10px]'>
            {collection.id}
          </span>
        </div>
      );
    },
    meta: {
      label: 'Collection ID',
      placeholder: 'Filter by ID...',
      variant: 'text',
      icon: Text
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
        case 'captured':
          return (
            <Badge
              variant='outline'
              className='border-indigo-500/30 bg-indigo-50 text-indigo-700 capitalize dark:bg-indigo-950/40 dark:text-indigo-300'
            >
              <CreditCard className='mr-1 h-3 w-3' />
              Captured
            </Badge>
          );
        case 'authorized':
          return (
            <Badge
              variant='outline'
              className='border-sky-500/30 bg-sky-50 text-sky-700 capitalize dark:bg-sky-950/40 dark:text-sky-300'
            >
              <ShieldCheck className='mr-1 h-3 w-3' />
              Authorized
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
      options: PAYMENT_STATUS_OPTIONS
    }
  },
  {
    id: 'amount',
    accessorKey: 'amount',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Amount' />
    ),
    cell: ({ row }) => {
      const collection = row.original;
      return (
        <div className='flex flex-col'>
          <span className='text-foreground text-xs font-semibold tracking-tight'>
            {collection.amount} {collection.currencyCode}
          </span>
          <span className='text-muted-foreground text-[10px]'>
            captured {collection.capturedAmount} · refunded{' '}
            {collection.refundedAmount}
          </span>
        </div>
      );
    }
  },
  {
    id: 'providerId',
    accessorKey: 'providerId',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Provider' />
    ),
    cell: ({ getValue }) => {
      const val = getValue() as string | null;
      return (
        <span className='text-muted-foreground text-xs'>
          {val ?? '—'}
        </span>
      );
    }
  },
  {
    id: 'orderId',
    accessorKey: 'orderId',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Order' />
    ),
    cell: ({ getValue }) => {
      const val = getValue() as string | null;
      if (!val) return <span className='text-muted-foreground text-xs'>—</span>;
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
    id: 'counts',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Payments / Refunds' />
    ),
    cell: ({ row }) => {
      const collection = row.original;
      return (
        <span className='text-muted-foreground text-xs'>
          {collection.payments?.length ?? 0} / {collection.refunds?.length ?? 0}
        </span>
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
