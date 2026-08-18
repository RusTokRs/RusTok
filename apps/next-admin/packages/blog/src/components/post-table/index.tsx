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
import { parseAsInteger, useQueryState } from 'nuqs';

interface PostTableParams<
  TData extends RowData = any,
  TValue extends CellData = any
> {
  data: TData[];
  totalItems: number;
  columns: ColumnDef<StockFeatures, TData, TValue>[];
}

export function PostTable<
  TData extends RowData = any,
  TValue extends CellData = any
>({ data, totalItems, columns }: PostTableParams<TData, TValue>) {
  const [pageSize] = useQueryState('perPage', parseAsInteger.withDefault(20));
  const pageCount = Math.ceil(totalItems / pageSize);

  const { table } = useDataTable({
    data,
    columns,
    pageCount,
    shallow: false,
    debounceMs: 500
  });

  return (
    <DataTable table={table}>
      <DataTableToolbar table={table} />
    </DataTable>
  );
}
