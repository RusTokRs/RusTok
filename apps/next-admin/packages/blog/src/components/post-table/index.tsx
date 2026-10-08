'use client';

import { DataTable } from '@/components/ui/table/data-table';
import { DataTableToolbar } from '@/components/ui/table/data-table-toolbar';
import { useDataTable } from '@/shared/hooks/use-data-table';
import type {
  CellData,
  ColumnDef,
  RowData,
  StockFeatures
} from '@tanstack/react-table';
import Link from 'next/link';

export interface PostTablePager {
  previousHref: string | null;
  nextHref: string | null;
}

interface PostTableParams<
  TData extends RowData = any,
  TValue extends CellData = any
> {
  data: TData[];
  pager: PostTablePager;
  columns: ColumnDef<StockFeatures, TData, TValue>[];
}

export function PostTable<
  TData extends RowData = any,
  TValue extends CellData = any
>({ data, pager, columns }: PostTableParams<TData, TValue>) {
  // Cursor-paged: the page count is unknown, so the table never paginates on its own.
  const { table } = useDataTable({
    data,
    columns,
    pageCount: -1,
    shallow: false,
    debounceMs: 500
  });

  return (
    <DataTable
      table={table}
      pagination={<CursorPagination pager={pager} />}
    >
      <DataTableToolbar table={table} />
    </DataTable>
  );
}

function CursorPagination({ pager }: { pager: PostTablePager }) {
  return (
    <nav aria-label='Posts pages' className='flex items-center justify-end gap-2'>
      <PagerLink href={pager.previousHref} label='Previous' />
      <PagerLink href={pager.nextHref} label='Next' />
    </nav>
  );
}

function PagerLink({ href, label }: { href: string | null; label: string }) {
  if (!href) {
    return (
      <span className='text-muted-foreground px-3 py-1.5 text-sm opacity-50'>
        {label}
      </span>
    );
  }
  return (
    <Link
      href={href}
      className='hover:bg-muted rounded-md border px-3 py-1.5 text-sm'
    >
      {label}
    </Link>
  );
}
