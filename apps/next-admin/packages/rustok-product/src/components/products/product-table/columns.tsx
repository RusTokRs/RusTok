'use client';

import * as React from 'react';
import type { ColumnDef, StockFeatures } from '@tanstack/react-table';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Checkbox } from '@/shared/ui/shadcn/checkbox';
import { DataTableColumnHeader } from '@/widgets/data-table/data-table-column-header';
import { CircleDot, Text } from 'lucide-react';
import Link from 'next/link';
import type { ProductListItem } from '../../../api/types';
import { CellAction } from './cell-action';

export const STATUS_OPTIONS = [
  { label: 'Published / Active', value: 'published' },
  { label: 'Active', value: 'active' },
  { label: 'Draft', value: 'draft' },
  { label: 'Archived', value: 'archived' }
];

export const columns: ColumnDef<StockFeatures, ProductListItem, any>[] = [
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
    id: 'title',
    accessorKey: 'title',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Product' />
    ),
    cell: ({ row }) => {
      const product = row.original;
      return (
        <div className='flex flex-col'>
          <Link
            href={`/dashboard/product/${product.id}`}
            className='hover:text-primary text-foreground max-w-[280px] truncate font-medium transition hover:underline'
          >
            {product.title || product.handle}
          </Link>
          <span className='text-muted-foreground font-mono text-xs'>
            {product.handle}
          </span>
        </div>
      );
    },
    meta: {
      label: 'Product',
      placeholder: 'Filter by title...',
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
      const status = String(getValue() ?? '').toLowerCase();
      if (status === 'active' || status === 'published') {
        return (
          <Badge
            variant='outline'
            className='border-emerald-500/30 bg-emerald-50 text-emerald-700 capitalize dark:bg-emerald-950/40 dark:text-emerald-300'
          >
            <CircleDot className='mr-1 h-3 w-3' />
            {status}
          </Badge>
        );
      }
      if (status === 'draft') {
        return (
          <Badge variant='secondary' className='capitalize'>
            <CircleDot className='mr-1 h-3 w-3' />
            {status}
          </Badge>
        );
      }
      return (
        <Badge variant='outline' className='text-muted-foreground capitalize'>
          <CircleDot className='mr-1 h-3 w-3' />
          {status}
        </Badge>
      );
    },
    enableColumnFilter: true,
    meta: {
      label: 'Status',
      variant: 'multiSelect',
      options: STATUS_OPTIONS
    }
  },
  {
    id: 'productType',
    accessorKey: 'productType',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Type' />
    ),
    cell: ({ getValue }) => {
      const val = getValue() as string | null;
      return val ? (
        <Badge variant='outline' className='text-xs font-normal'>
          {val}
        </Badge>
      ) : (
        <span className='text-muted-foreground text-xs'>—</span>
      );
    }
  },
  {
    id: 'vendor',
    accessorKey: 'vendor',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Vendor' />
    ),
    cell: ({ getValue }) => {
      const val = getValue() as string | null;
      return <span className='text-foreground/90 text-sm'>{val || '—'}</span>;
    },
    meta: {
      label: 'Vendor',
      placeholder: 'Filter by vendor...',
      variant: 'text'
    },
    enableColumnFilter: true
  },
  {
    id: 'publishedAt',
    accessorKey: 'publishedAt',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Published' />
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
