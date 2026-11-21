'use client';

import { DataTableShell } from '@/widgets/data-table/data-table-shell';
import type { AdminFulfillment } from '../../types';
import { columns } from './columns';

interface FulfillmentsTableProps {
  data: AdminFulfillment[];
  totalItems: number;
}

export function FulfillmentsTable({ data, totalItems }: FulfillmentsTableProps) {
  return (
    <DataTableShell data={data} columns={columns} totalItems={totalItems} />
  );
}
