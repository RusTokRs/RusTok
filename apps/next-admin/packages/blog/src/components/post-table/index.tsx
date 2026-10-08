'use client';

import type {
  CellData,
  ColumnDef,
  RowData,
  StockFeatures
} from '@tanstack/react-table';

import { DataTableShell } from '@/widgets/data-table/data-table-shell';

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
  return (
    <DataTableShell data={data} columns={columns} totalItems={totalItems} />
  );
}
