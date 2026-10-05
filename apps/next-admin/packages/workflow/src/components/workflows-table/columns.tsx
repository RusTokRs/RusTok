'use client';

import * as React from 'react';
import type { ColumnDef, StockFeatures } from '@tanstack/react-table';
import Link from 'next/link';
import { DataTableColumnHeader } from '@/widgets/data-table/data-table-column-header';
import type { WorkflowSummary } from '../../api/workflows';

export const WORKFLOW_STATUS_OPTIONS = [
  { label: 'Active', value: 'ACTIVE' },
  { label: 'Paused', value: 'PAUSED' },
  { label: 'Draft', value: 'DRAFT' },
  { label: 'Archived', value: 'ARCHIVED' }
];

export function statusClass(status: string): string {
  switch (status) {
    case 'ACTIVE':
      return 'bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400';
    case 'PAUSED':
      return 'bg-yellow-50 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400';
    case 'ARCHIVED':
      return 'bg-muted text-muted-foreground';
    default:
      return 'bg-primary/10 text-primary';
  }
}

export const columns: ColumnDef<StockFeatures, WorkflowSummary, any>[] = [
  {
    id: 'name',
    accessorKey: 'name',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Name' />
    ),
    cell: ({ row }) => {
      const wf = row.original;
      return (
        <Link
          href={`/dashboard/workflows/${wf.id}`}
          className='font-medium text-foreground hover:underline'
        >
          {wf.name}
        </Link>
      );
    },
    enableColumnFilter: true,
    meta: {
      label: 'Name'
    }
  },
  {
    id: 'status',
    accessorKey: 'status',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Status' />
    ),
    cell: ({ row }) => {
      const status = row.original.status;
      return (
        <span
          className={`inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-semibold ${statusClass(status)}`}
        >
          {status}
        </span>
      );
    },
    enableColumnFilter: true,
    meta: {
      label: 'Status',
      variant: 'select',
      options: WORKFLOW_STATUS_OPTIONS
    }
  },
  {
    id: 'failureCount',
    accessorKey: 'failureCount',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Failures' />
    ),
    cell: ({ row }) => (
      <span className='text-muted-foreground text-sm'>
        {row.original.failureCount}
      </span>
    ),
    meta: {
      label: 'Failures'
    }
  },
  {
    id: 'updatedAt',
    accessorKey: 'updatedAt',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Updated' />
    ),
    cell: ({ row }) => (
      <span className='text-muted-foreground text-xs'>
        {new Date(row.original.updatedAt).toLocaleDateString()}
      </span>
    ),
    meta: {
      label: 'Updated'
    }
  },
  {
    id: 'actions',
    header: () => <span className='sr-only'>Actions</span>,
    cell: ({ row }) => {
      const wf = row.original;
      return (
        <div className='text-right'>
          <Link
            href={`/dashboard/workflows/${wf.id}`}
            className='text-primary text-sm font-medium hover:underline'
          >
            View
          </Link>
        </div>
      );
    },
    enableSorting: false,
    enableHiding: false
  }
];
