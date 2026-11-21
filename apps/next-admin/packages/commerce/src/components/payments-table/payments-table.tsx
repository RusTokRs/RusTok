'use client';

import { DataTableShell } from '@/widgets/data-table/data-table-shell';
import type { AdminPaymentCollection } from '../../types';
import { columns } from './columns';

interface PaymentsTableProps {
  data: AdminPaymentCollection[];
  totalItems: number;
}

export function PaymentsTable({ data, totalItems }: PaymentsTableProps) {
  return (
    <DataTableShell data={data} columns={columns} totalItems={totalItems} />
  );
}
