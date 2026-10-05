'use client';

import * as React from 'react';
import type { ColumnDef, StockFeatures } from '@tanstack/react-table';
import type { WorkflowExecution } from '../api/workflows';
import { DataTable } from '@/widgets/data-table/data-table';
import { DataTableToolbar } from '@/widgets/data-table/data-table-toolbar';
import { DataTableColumnHeader } from '@/widgets/data-table/data-table-column-header';
import { useDataTable } from '@/shared/hooks/use-data-table';
import { parseAsInteger, useQueryState } from 'nuqs';

export const EXECUTION_STATUS_OPTIONS = [
  { label: 'Completed', value: 'COMPLETED' },
  { label: 'Failed', value: 'FAILED' },
  { label: 'Running', value: 'RUNNING' },
  { label: 'Timed out', value: 'TIMED_OUT' }
];

export function statusClass(status: string): string {
  switch (status) {
    case 'COMPLETED':
      return 'bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400';
    case 'FAILED':
      return 'bg-destructive/10 text-destructive';
    case 'RUNNING':
      return 'bg-primary/10 text-primary';
    case 'TIMED_OUT':
      return 'bg-orange-50 text-orange-700 dark:bg-orange-900/30 dark:text-orange-400';
    default:
      return 'bg-muted text-muted-foreground';
  }
}

export const executionColumns: ColumnDef<
  StockFeatures,
  WorkflowExecution,
  any
>[] = [
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
      options: EXECUTION_STATUS_OPTIONS
    }
  },
  {
    id: 'startedAt',
    accessorKey: 'startedAt',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Started' />
    ),
    cell: ({ row }) => (
      <span className='text-muted-foreground text-xs'>
        {new Date(row.original.startedAt).toLocaleString()}
      </span>
    ),
    meta: {
      label: 'Started'
    }
  },
  {
    id: 'completedAt',
    accessorKey: 'completedAt',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Completed' />
    ),
    cell: ({ row }) => (
      <span className='text-muted-foreground text-xs'>
        {row.original.completedAt
          ? new Date(row.original.completedAt).toLocaleString()
          : '—'}
      </span>
    ),
    meta: {
      label: 'Completed'
    }
  },
  {
    id: 'steps',
    header: 'Steps',
    cell: ({ row }) => (
      <span className='text-muted-foreground text-sm'>
        {row.original.stepExecutions.length}
      </span>
    )
  },
  {
    id: 'error',
    accessorKey: 'error',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Error' />
    ),
    cell: ({ row }) => (
      <span className='text-destructive line-clamp-1 max-w-xs text-xs'>
        {row.original.error ?? '—'}
      </span>
    ),
    meta: {
      label: 'Error'
    }
  }
];

interface ExecutionHistoryProps {
  executions: WorkflowExecution[];
}

export function ExecutionHistory({ executions }: ExecutionHistoryProps) {
  const [pageSize] = useQueryState('perPage', parseAsInteger.withDefault(10));
  const pageCount = Math.max(1, Math.ceil(executions.length / pageSize));

  const { table } = useDataTable({
    data: executions,
    columns: executionColumns,
    pageCount,
    shallow: false,
    debounceMs: 300
  });

  if (executions.length === 0) {
    return <p className='text-muted-foreground text-sm'>No executions yet.</p>;
  }

  return (
    <DataTable table={table}>
      <DataTableToolbar table={table} />
    </DataTable>
  );
}
