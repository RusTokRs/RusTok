'use client';

import * as React from 'react';

import { Button } from '@/shared/ui/shadcn/button';
import { Skeleton } from '@/shared/ui/shadcn/skeleton';
import { cn } from '@/shared/lib/utils';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from './table';

/**
 * The one table body every non-interactive admin table uses.
 *
 * Small tables used to hand-roll the same markup in every module: a header row per column, a
 * full-width empty state with `colSpan`, the row loop, and a skeleton while loading. Keeping that in
 * one component means a table declares *what* its columns are and *what* a cell renders — nothing
 * else — and every module gets the same loading, empty and accessibility behaviour.
 *
 * A table that needs client state per row (selection, inline editing) keeps using the stateful
 * `DataTable`/`DataTableShell` pair instead; this primitive is for the read-mostly case.
 */
export type DataTableStaticColumn<T> = {
  /** Stable key of the column; also used for the header and cell keys. */
  id: string;
  header: React.ReactNode;
  headerClassName?: string;
  cellClassName?: string;
  cell: (row: T) => React.ReactNode;
};

export interface DataTableStaticProps<T> {
  rows: T[];
  columns: DataTableStaticColumn<T>[];
  /** React key for a row; defaults to the row index, which is only safe for static lists. */
  getRowKey?: (row: T) => React.Key;
  /** Rendered across the full table width when there are no rows. */
  emptyState?: React.ReactNode;
  /** Renders skeleton cells instead of rows. */
  isLoading?: boolean;
  skeletonRows?: number;
  /** Wrapper class names; the bordered surface is the default. */
  className?: string;
}

export function DataTableStatic<T>({
  rows,
  columns,
  getRowKey,
  emptyState,
  isLoading,
  skeletonRows = 5,
  className
}: DataTableStaticProps<T>) {
  return (
    <div className={cn('rounded-md border', className)}>
      <Table>
        <TableHeader>
          <TableRow>
            {columns.map((column) => (
              <TableHead key={column.id} className={column.headerClassName}>
                {column.header}
              </TableHead>
            ))}
          </TableRow>
        </TableHeader>
        <TableBody>
          {isLoading ? (
            Array.from({ length: skeletonRows }, (_, index) => (
              <TableRow key={`skeleton-${index}`}>
                {columns.map((column) => (
                  <TableCell key={column.id}>
                    <Skeleton className='h-4 w-full' />
                  </TableCell>
                ))}
              </TableRow>
            ))
          ) : rows.length === 0 ? (
            <TableRow>
              <TableCell
                colSpan={columns.length}
                className='text-muted-foreground py-8 text-center text-sm'
              >
                {emptyState ?? 'No results.'}
              </TableCell>
            </TableRow>
          ) : (
            rows.map((row, index) => (
              <TableRow key={getRowKey ? getRowKey(row) : index}>
                {columns.map((column) => (
                  <TableCell key={column.id} className={column.cellClassName}>
                    {column.cell(row)}
                  </TableCell>
                ))}
              </TableRow>
            ))
          )}
        </TableBody>
      </Table>
    </div>
  );
}

/** The standard empty state: an icon above one line of copy. */
export function DataTableEmptyState({
  icon,
  message
}: {
  icon?: React.ReactNode;
  message: React.ReactNode;
}) {
  return (
    <div className='flex flex-col items-center justify-center gap-1.5'>
      {icon ? (
        <span className='text-muted-foreground/50 [&_svg]:h-8 [&_svg]:w-8'>
          {icon}
        </span>
      ) : null}
      <p>{message}</p>
    </div>
  );
}

/**
 * The one pagination bar for tables that paginate outside the route.
 *
 * Server-paginated tables used to lay out "Previous / page x of y / Next" themselves — and the
 * commerce templates only drew the two buttons, without saying which page was shown. This bar keeps
 * the layout in one place and takes an optional range label for tables that report their window.
 */
export interface DataTablePaginationBarProps {
  /** 1-based page currently shown. */
  page: number;
  /** Total pages; at least 1, so a bar can render for a single page of results. */
  pageCount: number;
  onPageChange: (page: number) => void;
  disabled?: boolean;
  /** Left-hand copy, e.g. "Showing 1 to 20 of 137 rows"; built with `dataTableRangeLabel`. */
  rangeLabel?: React.ReactNode;
  className?: string;
}

export function dataTableRangeLabel({
  page,
  perPage,
  total,
  noun
}: {
  page: number;
  perPage: number;
  total: number;
  noun: string;
}): string {
  const first = total === 0 ? 0 : (page - 1) * perPage + 1;
  const last = Math.min(page * perPage, total);
  return `Showing ${first} to ${last} of ${total} ${noun}`;
}

export function DataTablePaginationBar({
  page,
  pageCount,
  onPageChange,
  disabled,
  rangeLabel,
  className
}: DataTablePaginationBarProps) {
  const totalPages = Math.max(1, pageCount);
  return (
    <div
      className={cn(
        'text-muted-foreground mt-4 flex items-center justify-between text-xs',
        className
      )}
    >
      <span>{rangeLabel}</span>
      <div className='ml-auto flex items-center gap-1'>
        <Button
          variant='outline'
          size='sm'
          className='h-8 text-xs'
          onClick={() => onPageChange(page - 1)}
          disabled={disabled || page <= 1}
        >
          Previous
        </Button>
        <span className='px-2 font-mono'>
          {page} / {totalPages}
        </span>
        <Button
          variant='outline'
          size='sm'
          className='h-8 text-xs'
          onClick={() => onPageChange(page + 1)}
          disabled={disabled || page >= totalPages}
        >
          Next
        </Button>
      </div>
    </div>
  );
}
