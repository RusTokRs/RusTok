import * as React from 'react';
import { ariaSortForColumn } from '../sort';
import type { DataTableStaticProps } from './types';

/**
 * Canonical framework-provided static table renderer for RusTok.
 *
 * Provides consistent styling, sortable headers, accessible aria attributes,
 * skeleton loaders and empty states.
 */
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
  density = 'default'
}: DataTableStaticProps<T>) {
  const isCompact = density === 'compact';

  return (
    <div className={`rounded-md border ${className ?? ''}`}>
      <div className='relative w-full overflow-auto'>
        <table className='w-full caption-bottom text-sm' aria-busy={isLoading || undefined}>
          <thead className='[&_tr]:border-b'>
            <tr className='border-b transition-colors'>
              {columns.map((column) => {
                const isCurrentSort = sort?.columnId === column.id;
                const isSortable = Boolean(column.sortable && onSort);
                const ariaSort = ariaSortForColumn(
                  sort ?? { columnId: null, direction: null },
                  column.id
                );

                return (
                  <th
                    key={column.id}
                    scope='col'
                    aria-sort={ariaSort}
                    className={`text-muted-foreground text-left align-middle font-medium ${
                      isCompact ? 'h-8 px-2.5 py-1 text-xs' : 'h-10 px-4 py-2'
                    } ${column.headerClassName ?? ''}`}
                  >
                    {isSortable ? (
                      <button
                        type='button'
                        onClick={() => onSort?.(column.id)}
                        className='text-muted-foreground hover:text-foreground -ml-1 inline-flex items-center gap-1 rounded px-1 py-0.5 text-xs font-medium transition-colors focus:outline-none focus:ring-1 focus:ring-ring'
                      >
                        <span>{column.header}</span>
                        {isCurrentSort ? (
                          sort?.direction === 'desc' ? (
                            <span aria-hidden='true'>&darr;</span>
                          ) : (
                            <span aria-hidden='true'>&uarr;</span>
                          )
                        ) : (
                          <span aria-hidden='true' className='opacity-40'>
                            &#8645;
                          </span>
                        )}
                      </button>
                    ) : (
                      column.header
                    )}
                  </th>
                );
              })}
            </tr>
          </thead>
          <tbody className='[&_tr:last-child]:border-0'>
            {isLoading ? (
              Array.from({ length: skeletonRows }, (_, index) => (
                <tr key={`skeleton-${index}`} className='border-b transition-colors'>
                  {columns.map((column) => (
                    <td
                      key={column.id}
                      className={`align-middle ${isCompact ? 'px-2.5 py-1.5' : 'p-4'}`}
                    >
                      <div
                        className={`animate-pulse rounded bg-muted ${
                          isCompact ? 'h-3.5' : 'h-4'
                        } w-full`}
                      />
                    </td>
                  ))}
                </tr>
              ))
            ) : rows.length === 0 ? (
              <tr>
                <td
                  colSpan={Math.max(1, columns.length)}
                  className={`text-muted-foreground text-center text-sm ${
                    isCompact ? 'py-4' : 'py-8'
                  }`}
                >
                  {emptyState ?? 'No results.'}
                </td>
              </tr>
            ) : (
              rows.map((row, index) => (
                <tr
                  key={getRowKey ? getRowKey(row) : index}
                  className='hover:bg-muted/50 border-b transition-colors'
                >
                  {columns.map((column) => (
                    <td
                      key={column.id}
                      className={`align-middle ${
                        isCompact ? 'px-2.5 py-1.5 text-xs' : 'p-4'
                      } ${column.cellClassName ?? ''}`}
                    >
                      {column.cell(row)}
                    </td>
                  ))}
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
