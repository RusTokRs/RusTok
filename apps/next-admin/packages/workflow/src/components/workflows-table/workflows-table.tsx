'use client';

import * as React from 'react';
import type { WorkflowSummary } from '../../api/workflows';
import { DataTable } from '@/widgets/data-table/data-table';
import { DataTableToolbar } from '@/widgets/data-table/data-table-toolbar';
import { useDataTable } from '@/shared/hooks/use-data-table';
import { parseAsInteger, useQueryState } from 'nuqs';
import { columns } from './columns';

interface WorkflowsTableProps {
  data: WorkflowSummary[];
}

export function WorkflowsTable({ data }: WorkflowsTableProps) {
  const [pageSize] = useQueryState('perPage', parseAsInteger.withDefault(10));
  const pageCount = Math.max(1, Math.ceil(data.length / pageSize));

  const { table } = useDataTable({
    data,
    columns,
    pageCount,
    shallow: false,
    debounceMs: 300
  });

  return (
    <DataTable table={table}>
      <DataTableToolbar table={table} />
    </DataTable>
  );
}
