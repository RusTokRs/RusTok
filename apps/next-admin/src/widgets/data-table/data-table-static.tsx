'use client';

import * as React from 'react';

import {
  CaretSortIcon,
  ChevronDownIcon,
  ChevronUpIcon
} from '@radix-ui/react-icons';
import { Button } from '@/shared/ui/shadcn/button';
import { Skeleton } from '@/shared/ui/shadcn/skeleton';
import { cn } from '@/shared/lib/utils';
import {
  DEFAULT_PAGE_SIZE,
  ariaSortForColumn,
  clampPage,
  dataTableRangeLabel as gridRangeLabel,
  emptySortState
} from '@rustok/ui-grid';
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
  /** Enables clicking this header to trigger `onSort`. */
  sortable?: boolean;
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
  /** Active sort state: columnId and asc/desc direction. */
  sort?: { columnId: string; direction: 'asc' | 'desc' } | null;
  /** Callback invoked when a sortable column header is clicked. */
  onSort?: (columnId: string) => void;
  /** Controls table row padding density. Defaults to 'default'. */
  density?: 'default' | 'compact';
  /**
   * Accessible name of the scrollable table region. Set it on tables whose columns do not announce
   * themselves (a wide, horizontally scrolled table is otherwise impossible to orient in).
   */
  ariaLabel?: string;
}

export function DataTableStatic<T>({
  rows,
  columns,
  getRowKey,
  emptyState,
  isLoading,
  skeletonRows = 5,
  className,
  sort,
  onSort,
  density = 'default',
  ariaLabel
}: DataTableStaticProps<T>) {
  const isCompact = density === 'compact';
  const sortState = sort ?? emptySortState();

  return (
    <div
      className={cn('rounded-md border', className)}
      role={ariaLabel ? 'region' : undefined}
      aria-label={ariaLabel}
      aria-busy={isLoading || undefined}
    >
      <Table>
        <TableHeader>
          <TableRow>
            {columns.map((column) => {
              const isCurrentSort = sort?.columnId === column.id;
              const isSortable = Boolean(column.sortable && onSort);

              return (
                <TableHead
                  key={column.id}
                  // `aria-sort` is the machine-readable counterpart of the arrow glyph: it reports
                  // the direction even on a column whose header only carries text.
                  aria-sort={isSortable ? ariaSortForColumn(sortState, column.id) : undefined}
                  className={cn(
                    isCompact && 'h-8 px-2.5 py-1 text-xs',
                    column.headerClassName
                  )}
                >
                  {isSortable ? (
                    <button
                      type='button'
                      onClick={() => onSort?.(column.id)}
                      className='text-muted-foreground hover:text-foreground focus:ring-ring -ml-1 inline-flex items-center gap-1 rounded px-1 py-0.5 text-xs font-medium transition-colors focus:ring-1 focus:outline-none'
                    >
                      <span>{column.header}</span>
                      {isCurrentSort ? (
                        sort?.direction === 'desc' ? (
                          <ChevronDownIcon className='h-3.5 w-3.5' />
                        ) : (
                          <ChevronUpIcon className='h-3.5 w-3.5' />
                        )
                      ) : (
                        <CaretSortIcon className='h-3.5 w-3.5 opacity-40' />
                      )}
                    </button>
                  ) : (
                    column.header
                  )}
                </TableHead>
              );
            })}
          </TableRow>
        </TableHeader>
        <TableBody>
          {isLoading ? (
            Array.from({ length: skeletonRows }, (_, index) => (
              <TableRow key={`skeleton-${index}`}>
                {columns.map((column) => (
                  <TableCell
                    key={column.id}
                    className={isCompact ? 'px-2.5 py-1.5' : undefined}
                  >
                    <Skeleton
                      className={cn('w-full', isCompact ? 'h-3.5' : 'h-4')}
                    />
                  </TableCell>
                ))}
              </TableRow>
            ))
          ) : rows.length === 0 ? (
            <TableRow>
              <TableCell
                // At least one, so a table with a header but no column definitions still has a cell
                // to hold the empty state instead of rendering a broken row.
                colSpan={Math.max(1, columns.length)}
                className={cn(
                  'text-muted-foreground text-center text-sm',
                  isCompact ? 'py-4' : 'py-8'
                )}
              >
                {emptyState ?? 'No results.'}
              </TableCell>
            </TableRow>
          ) : (
            rows.map((row, index) => (
              <TableRow key={getRowKey ? getRowKey(row) : index}>
                {columns.map((column) => (
                  <TableCell
                    key={column.id}
                    className={cn(
                      isCompact && 'px-2.5 py-1.5 text-xs',
                      column.cellClassName
                    )}
                  >
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
  /** Left-hand copy, e.g. "Showing 1–20 of 137 orders"; built with `dataTableRangeLabel`. */
  rangeLabel?: React.ReactNode;
  className?: string;
}

/**
 * The one "Showing … of … rows" sentence every admin table writes.
 *
 * The math itself is not here: `dataTableRangeLabel` in `@rustok/ui-grid` clamps the page into the
 * real page count and resolves an out-of-range page size, so a table fed `page=99` by a hand-edited
 * URL reports the last window instead of "Showing 1961 to 20 of 20 rows". This function only adds the
 * leading word and the row noun the shared helper does not know about.
 */
export function dataTableRangeLabel({
  page,
  perPage = DEFAULT_PAGE_SIZE,
  total,
  noun
}: {
  page: number;
  perPage?: number;
  total: number;
  noun: string;
}): string {
  return `Showing ${gridRangeLabel(clampPage(page, total, perPage), perPage, total)} ${noun}`;
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
