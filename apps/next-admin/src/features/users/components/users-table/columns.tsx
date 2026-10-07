'use client';

import * as React from 'react';
import type { ColumnDef, StockFeatures } from '@tanstack/react-table';
import { Badge } from '@/shared/ui/shadcn/badge';
import { Checkbox } from '@/shared/ui/shadcn/checkbox';
import { DataTableColumnHeader } from '@/widgets/data-table/data-table-column-header';
import { CircleDot, Shield, Text } from 'lucide-react';
import Link from 'next/link';
import type { User } from '@/entities/user';
import { CellAction } from './cell-action';
import { useTranslations } from '@rustok/next-fluent';

export const ROLE_OPTIONS = [
  { label: 'Super Admin', value: 'SUPER_ADMIN' },
  { label: 'Admin', value: 'ADMIN' },
  { label: 'Manager', value: 'MANAGER' },
  { label: 'Customer', value: 'CUSTOMER' }
];

export const STATUS_OPTIONS = [
  { label: 'Active', value: 'ACTIVE' },
  { label: 'Inactive', value: 'INACTIVE' },
  { label: 'Suspended', value: 'SUSPENDED' },
  { label: 'Pending', value: 'PENDING' }
];

const SelectHeader: React.FC<{ table: any }> = ({ table }) => {
  const t = useTranslations('users');
  return (
    <Checkbox
      checked={
        table.getIsAllPageRowsSelected() ||
        (table.getIsSomePageRowsSelected() && 'indeterminate')
      }
      onCheckedChange={(value) => table.toggleAllPageRowsSelected(!!value)}
      aria-label={t('table.select.all')}
      className='translate-y-0.5'
    />
  );
};

const SelectCell: React.FC<{ row: any }> = ({ row }) => {
  const t = useTranslations('users');
  return (
    <Checkbox
      checked={row.getIsSelected()}
      onCheckedChange={(value) => row.toggleSelected(!!value)}
      aria-label={t('table.select.row')}
      className='translate-y-0.5'
    />
  );
};

const EmailHeader: React.FC<{ column: any }> = ({ column }) => {
  const t = useTranslations('users');
  return <DataTableColumnHeader column={column} title={t('table.email.header')} />;
};

const RoleHeader: React.FC<{ column: any }> = ({ column }) => {
  const t = useTranslations('users');
  return <DataTableColumnHeader column={column} title={t('table.role.header')} />;
};

const StatusHeader: React.FC<{ column: any }> = ({ column }) => {
  const t = useTranslations('users');
  return <DataTableColumnHeader column={column} title={t('table.status.header')} />;
};

const CreatedHeader: React.FC<{ column: any }> = ({ column }) => {
  const t = useTranslations('users');
  return <DataTableColumnHeader column={column} title={t('table.created.header')} />;
};

export const columns: ColumnDef<StockFeatures, User, any>[] = [
  {
    id: 'select',
    header: SelectHeader,
    cell: SelectCell,
    enableSorting: false,
    enableHiding: false,
    size: 40
  },
  {
    id: 'email',
    accessorKey: 'email',
    header: EmailHeader,
    cell: ({ row }) => {
      const user = row.original;
      return (
        <div className='flex flex-col'>
          <Link
            href={`/dashboard/users/${user.id}`}
            className='hover:text-primary max-w-[280px] truncate font-medium text-foreground transition hover:underline'
          >
            {user.email}
          </Link>
          {user.name && (
            <span className='text-muted-foreground text-xs'>{user.name}</span>
          )}
        </div>
      );
    },
    meta: {
      label: 'Email',
      placeholder: 'Search by email or name...',
      variant: 'text',
      icon: Text
    },
    enableColumnFilter: true
  },
  {
    id: 'role',
    accessorKey: 'role',
    header: RoleHeader,
    cell: ({ getValue }) => {
      const role = String(getValue() ?? '').toUpperCase();
      const isSuperAdmin = role === 'SUPER_ADMIN';
      const isAdmin = role === 'ADMIN';
      const isManager = role === 'MANAGER';
      return (
        <Badge
          variant={
            isSuperAdmin
              ? 'destructive'
              : isAdmin
                ? 'default'
                : isManager
                  ? 'secondary'
                  : 'outline'
          }
          className='gap-1 text-xs'
        >
          <Shield className='h-3 w-3' />
          {role}
        </Badge>
      );
    },
    enableColumnFilter: true,
    meta: {
      label: 'Role',
      variant: 'multiSelect',
      options: ROLE_OPTIONS
    }
  },
  {
    id: 'status',
    accessorKey: 'status',
    header: StatusHeader,
    cell: ({ getValue }) => {
      const status = String(getValue() ?? '').toUpperCase();
      const isActive = status === 'ACTIVE';
      return (
        <Badge
          variant={isActive ? 'outline' : 'secondary'}
          className={
            isActive
              ? 'border-emerald-500/30 bg-emerald-50 text-emerald-700 capitalize dark:bg-emerald-950/40 dark:text-emerald-300'
              : 'capitalize'
          }
        >
          <CircleDot className='mr-1 h-3 w-3' />
          {status.toLowerCase()}
        </Badge>
      );
    },
    enableColumnFilter: true,
    meta: {
      label: 'Status',
      variant: 'multiSelect',
      options: STATUS_OPTIONS
    }
  },
  {
    id: 'createdAt',
    accessorKey: 'createdAt',
    header: CreatedHeader,
    cell: ({ getValue }) => {
      const raw = getValue() as string | null;
      if (!raw) return <span className='text-muted-foreground text-xs'>—</span>;
      return (
        <span className='text-muted-foreground whitespace-nowrap text-xs'>
          {new Date(raw).toLocaleDateString()}
        </span>
      );
    }
  },
  {
    id: 'actions',
    cell: ({ row }) => <CellAction data={row.original} />,
    size: 50
  }
];

