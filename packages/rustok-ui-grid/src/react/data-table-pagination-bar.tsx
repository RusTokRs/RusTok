import * as React from 'react';
import type { DataTablePaginationBarProps } from './types';

/**
 * Accessible pagination bar for tables that paginate outside route state.
 */
export function DataTablePaginationBar({
  page,
  pageCount,
  onPageChange,
  rangeLabel,
  disabled = false,
  className
}: DataTablePaginationBarProps) {
  const safePageCount = Math.max(1, pageCount);
  const isFirst = page <= 1;
  const isLast = page >= safePageCount;

  return (
    <div
      className={`flex items-center justify-between gap-4 px-2 py-4 ${className ?? ''}`}
      aria-label='Pagination'
    >
      <div className='text-muted-foreground text-xs'>
        {rangeLabel ?? `Page ${page} of ${safePageCount}`}
      </div>
      <div className='flex items-center gap-2'>
        <button
          type='button'
          disabled={disabled || isFirst}
          onClick={() => onPageChange(page - 1)}
          className='inline-flex h-8 items-center justify-center rounded-md border border-input bg-background px-3 text-xs font-medium transition-colors hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-50'
          aria-label='Previous page'
        >
          Previous
        </button>
        <span className='text-muted-foreground text-xs'>
          {page} / {safePageCount}
        </span>
        <button
          type='button'
          disabled={disabled || isLast}
          onClick={() => onPageChange(page + 1)}
          className='inline-flex h-8 items-center justify-center rounded-md border border-input bg-background px-3 text-xs font-medium transition-colors hover:bg-accent hover:text-accent-foreground disabled:pointer-events-none disabled:opacity-50'
          aria-label='Next page'
        >
          Next
        </button>
      </div>
    </div>
  );
}
