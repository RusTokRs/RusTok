'use client';

import { DataTableShell } from '@/widgets/data-table/data-table-shell';
import type { ProductListItem } from '../../../api/types';
import { columns } from './columns';

interface ProductTableProps {
  data: ProductListItem[];
  totalItems: number;
}

export function ProductTable({ data, totalItems }: ProductTableProps) {
  return (
    <DataTableShell data={data} columns={columns} totalItems={totalItems} />
  );
}
