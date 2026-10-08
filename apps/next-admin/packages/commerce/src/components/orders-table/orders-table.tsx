'use client';

import { DataTableShell } from '@/widgets/data-table/data-table-shell';
import type { OrderListItem } from '../../types';
import { columns } from './columns';

interface OrdersTableProps {
  data: OrderListItem[];
  totalItems: number;
}

export function OrdersTable({ data, totalItems }: OrdersTableProps) {
  return (
    <DataTableShell data={data} columns={columns} totalItems={totalItems} />
  );
}
