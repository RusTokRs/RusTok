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
  XCircle,
  RotateCcw,
  Text
} from 'lucide-react';
import type { AdminRefund } from '../../types';

export const REFUND_STATUS_OPTIONS = [
  { label: 'Pending', value: 'pending' },
  { label: 'Refunded', value: 'refunded' },
  { label: 'Cancelled', value: 'cancelled' }
];

export const columns: ColumnDef<StockFeatures, AdminRefund, any>[] = [
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
      <DataTableColumnHeader column={column} title='Refund' />
    ),
    cell: ({ row }) => {
      const refund = row.original;
      const shortId = refund.id.slice(0, 8);
      return (
        <div className='flex flex-col'>
          <span className='text-foreground max-w-[200px] truncate font-mono text-xs font-semibold'>
            #{shortId}
          </span>
          <span className='text-muted-foreground font-mono text-[10px]'>
            {refund.id}
          </span>
        </div>
      );
    },
    meta: {
      label: 'Refund ID',
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
        case 'refunded':
          return (
            <Badge
              variant='outline'
              className='border-emerald-500/30 bg-emerald-50 text-emerald-700 capitalize dark:bg-emerald-950/40 dark:text-emerald-300'
            >
              <CheckCircle2 className='mr-1 h-3 w-3' />
              Refunded
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
      options: REFUND_STATUS_OPTIONS
    }
  },
  {
    id: 'amount',
    accessorKey: 'amount',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Amount' />
    ),
    cell: ({ row }) => {
      const refund = row.original;
      return (
        <span className='text-foreground text-xs font-semibold tracking-tight'>
          {refund.amount} {refund.currencyCode}
        </span>
      );
    }
  },
  {
    id: 'reason',
    accessorKey: 'reason',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Reason' />
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
    id: 'paymentCollectionId',
    accessorKey: 'paymentCollectionId',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Collection' />
    ),
    cell: ({ getValue }) => {
      const val = getValue() as string | null;
      return (
        <span
          className='text-muted-foreground font-mono text-[10px]'
          title={val ?? undefined}
        >
          {val ? `#${val.slice(0, 8)}` : '—'}
        </span>
      );
    }
  },
  {
    id: 'refundedAt',
    accessorKey: 'refundedAt',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Refunded' />
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
    cell: ({ row }) => (
      <span
        className='text-muted-foreground/60 inline-flex items-center gap-1 text-xs'
        title='Refunds follow their payment collection; open the order to inspect'
      >
        <RotateCcw className='h-3 w-3' />
      </span>
    ),
    size: 50
  }
];
