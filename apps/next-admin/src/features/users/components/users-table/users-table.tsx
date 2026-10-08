'use client';

import { DataTableShell } from '@/widgets/data-table/data-table-shell';
import type { User } from '@/entities/user';
import { columns } from './columns';

interface UsersTableProps {
  data: User[];
  totalItems: number;
}

export function UsersTable({ data, totalItems }: UsersTableProps) {
  return (
    <DataTableShell
      data={data}
      columns={columns}
      totalItems={totalItems}
      defaultPageSize={12}
    />
  );
}
