'use client';

import { DataTableShell } from '@/widgets/data-table/data-table-shell';
import type { AdminRefund } from '../../types';
import { columns } from './columns';

interface RefundsTableProps {
  data: AdminRefund[];
  totalItems: number;
}

export function RefundsTable({ data, totalItems }: RefundsTableProps) {
  return (
    <DataTableShell data={data} columns={columns} totalItems={totalItems} />
  );
}
