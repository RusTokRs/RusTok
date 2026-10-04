'use client';

import * as React from 'react';
import { DataTable } from '@/widgets/data-table/data-table';
import { DataTableToolbar } from '@/widgets/data-table/data-table-toolbar';
import { useDataTable } from '@/shared/hooks/use-data-table';
import { parseAsInteger, useQueryState } from 'nuqs';
import type { ProductListItem } from '../../../api/types';
import { columns } from './columns';

interface ProductTableProps {
  data: ProductListItem[];
  totalItems: number;
}

export function ProductTable({ data, totalItems }: ProductTableProps) {
  const [pageSize] = useQueryState('perPage', parseAsInteger.withDefault(20));
  const pageCount = Math.max(1, Math.ceil(totalItems / pageSize));

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
