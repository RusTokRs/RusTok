'use client';

import * as React from 'react';
import { DataTable } from '@/widgets/data-table/data-table';
import { DataTableToolbar } from '@/widgets/data-table/data-table-toolbar';
import { useDataTable } from '@/shared/hooks/use-data-table';
import { parseAsInteger, useQueryState } from 'nuqs';
import type { User } from '@/entities/user';
import { columns } from './columns';

interface UsersTableProps {
  data: User[];
  totalItems: number;
}

export function UsersTable({ data, totalItems }: UsersTableProps) {
  const [pageSize] = useQueryState('perPage', parseAsInteger.withDefault(12));
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
