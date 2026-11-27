import type { ReactTable, RowData, StockFeatures } from '@tanstack/react-table';
import { useTranslations } from '@rustok/next-fluent/client';
import { ChevronsLeft, ChevronsRight } from 'lucide-react';

import { Button } from '@/shared/ui/shadcn/button';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import { cn } from '@/shared/lib/utils';
import { ChevronLeftIcon, ChevronRightIcon } from '@radix-ui/react-icons';

interface DataTablePaginationProps<
  TData extends RowData = any
> extends React.ComponentProps<'div'> {
  table: ReactTable<StockFeatures, TData>;
  pageSizeOptions?: number[];
}

export function DataTablePagination<TData extends RowData = any>({
  table,
  pageSizeOptions = [10, 20, 30, 40, 50],
  className,
  ...props
}: DataTablePaginationProps<TData>) {
  const t = useTranslations('data-table');
  const { pageIndex, pageSize } = table.state.pagination;
  const selected = table.getFilteredSelectedRowModel().rows.length;
  const total = table.getRowCount();
  const availablePageSizes = [...new Set([...pageSizeOptions, pageSize])].sort(
    (left, right) => left - right
  );

  return (
    <div
      className={cn(
        'flex w-full flex-col-reverse items-center justify-between gap-4 overflow-auto p-1 sm:flex-row sm:gap-8',
        className
      )}
      {...props}
    >
      <div className='text-muted-foreground flex-1 text-sm whitespace-nowrap'>
        {selected > 0 ? t('selection', { selected, total }) : t('total', { total })}
      </div>
      <div className='flex flex-col-reverse items-center gap-4 sm:flex-row sm:gap-6 lg:gap-8'>
        <div className='flex items-center space-x-2'>
          <p className='text-sm font-medium whitespace-nowrap'>{t('rows-per-page')}</p>
          <Select
            value={String(pageSize)}
            onValueChange={(value) => {
              table.setPageSize(Number(value));
            }}
          >
            <SelectTrigger
              aria-label={t('rows-per-page')}
              className='h-8 w-[4.5rem] [&[data-size]]:h-8'
            >
              <SelectValue />
            </SelectTrigger>
            <SelectContent side='top'>
              {availablePageSizes.map((size) => (
                <SelectItem key={size} value={String(size)}>
                  {size}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <div className='flex items-center justify-center text-sm font-medium'>
          {t('page-of', { page: pageIndex + 1, pages: table.getPageCount() })}
        </div>
        <div className='flex items-center space-x-2'>
          <Button
            aria-label={t('first-page')}
            variant='outline'
            size='icon'
            className='hidden size-8 lg:flex'
            onClick={() => table.setPageIndex(0)}
            disabled={!table.getCanPreviousPage()}
          >
            <ChevronsLeft />
          </Button>
          <Button
            aria-label={t('previous-page')}
            variant='outline'
            size='icon'
            className='size-8'
            onClick={() => table.previousPage()}
            disabled={!table.getCanPreviousPage()}
          >
            <ChevronLeftIcon />
          </Button>
          <Button
            aria-label={t('next-page')}
            variant='outline'
            size='icon'
            className='size-8'
            onClick={() => table.nextPage()}
            disabled={!table.getCanNextPage()}
          >
            <ChevronRightIcon />
          </Button>
          <Button
            aria-label={t('last-page')}
            variant='outline'
            size='icon'
            className='hidden size-8 lg:flex'
            onClick={() => table.setPageIndex(table.getPageCount() - 1)}
            disabled={!table.getCanNextPage()}
          >
            <ChevronsRight />
          </Button>
        </div>
      </div>
    </div>
  );
}
