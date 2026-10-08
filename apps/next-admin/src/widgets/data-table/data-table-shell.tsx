'use client';

import {
  type ColumnDef,
  type RowData,
  type StockFeatures
} from '@tanstack/react-table';
import { parseAsInteger, useQueryState } from 'nuqs';
import * as React from 'react';

import { useDataTable } from '@/shared/hooks/use-data-table';
import { DataTable } from './data-table';
import { DataTableSkeleton } from './data-table-skeleton';
import { DataTableToolbar } from './data-table-toolbar';

/**
 * The one client shell every paginated admin table uses.
 *
 * Before this component each table repeated the same four decisions — read the page size from the
 * route, compute the page count, configure `useDataTable` (`shallow: false`, a debounce), and render
 * the toolbar — and they drifted: page sizes of 20/10/12, debounces of 500/300, and `Math.ceil(x/y)`
 * next to `Math.max(1, Math.ceil(x/y))`, which yields a page count of zero for an empty result set.
 * The shell keeps the decisions in one place, so a table only supplies its data, its columns and its
 * toolbar extras.
 */
export interface DataTableShellProps<TData extends RowData = any> {
  data: TData[];
  columns: ColumnDef<StockFeatures, TData, any>[];
  /**
   * Server-reported row total when the table paginates on the server; defaults to the rows given,
   * so a client-paginated table needs no second source of truth.
   */
  totalItems?: number;
  /** Rows per page before the user changes it; the route keeps the choice across reloads. */
  defaultPageSize?: number;
  /** Filter-input debounce before the route is rewritten. */
  debounceMs?: number;
  /** Renders the loading skeleton instead of the table. */
  isLoading?: boolean;
  /** Columns the skeleton draws; defaults to the column definitions given. */
  skeletonColumns?: number;
  /** Rows the skeleton draws. */
  skeletonRows?: number;
  /** Toolbar extras (bulk actions, "new" buttons) rendered inside the standard toolbar. */
  children?: React.ReactNode;
}

export function DataTableShell<TData extends RowData = any>({
  data,
  columns,
  totalItems,
  defaultPageSize = 20,
  debounceMs = 500,
  isLoading,
  skeletonColumns,
  skeletonRows = 5,
  children
}: DataTableShellProps<TData>) {
  const [pageSize] = useQueryState(
    'perPage',
    parseAsInteger.withDefault(defaultPageSize)
  );
  const rowCount = typeof totalItems === 'number' ? totalItems : data.length;
  // A table with no rows still has one page: `Math.ceil(0 / 20)` is 0, and a grid rendering zero
  // pages shows neither rows nor pagination controls.
  const pageCount = Math.max(1, Math.ceil(rowCount / Math.max(1, pageSize)));

  const { table } = useDataTable({
    data,
    columns,
    pageCount,
    shallow: false,
    debounceMs
  });

  if (isLoading) {
    return (
      <DataTableSkeleton
        columnCount={skeletonColumns ?? columns.length}
        rowCount={skeletonRows}
      />
    );
  }

  return (
    <DataTable table={table}>
      <DataTableToolbar table={table}>{children}</DataTableToolbar>
    </DataTable>
  );
}
