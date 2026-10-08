'use client';

import { DataTableShell } from '@/widgets/data-table/data-table-shell';
import type { WorkflowSummary } from '../../api/workflows';
import { columns } from './columns';

interface WorkflowsTableProps {
  data: WorkflowSummary[];
}

export function WorkflowsTable({ data }: WorkflowsTableProps) {
  return (
    <DataTableShell
      data={data}
      columns={columns}
      defaultPageSize={10}
      debounceMs={300}
    />
  );
}
