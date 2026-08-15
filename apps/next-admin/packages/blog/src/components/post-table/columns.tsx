'use client';

import { Badge } from '@/components/ui/badge';
import { DataTableColumnHeader } from '@/components/ui/table/data-table-column-header';
import { ColumnDef } from '@tanstack/react-table';
import { Text, CircleDot } from 'lucide-react';
import { CellAction } from './cell-action';
import { STATUS_OPTIONS } from './options';
import type { PostSummary } from '../../api/posts';

const statusVariant: Record<string, 'default' | 'secondary' | 'outline'> = {
  PUBLISHED: 'default',
  DRAFT: 'secondary',
  ARCHIVED: 'outline'
};

const statusLabel: Record<string, string> = {
  DRAFT: 'Draft',
  PUBLISHED: 'Published',
  ARCHIVED: 'Archived'
};

export const columns: ColumnDef<PostSummary, any>[] = [
  {
    id: 'title',
    accessorKey: 'title',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Title' />
    ),
    cell: ({ getValue }) => (
      <div className='max-w-[300px] truncate font-medium'>
        {String(getValue() ?? '')}
      </div>
    ),
    meta: {
      label: 'Title',
      placeholder: 'Search posts...',
      variant: 'text',
      icon: Text
    },
    enableColumnFilter: true
  },
  {
    id: 'status',
    accessorKey: 'status',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title='Status' />
    ),
    cell: ({ getValue }) => {
      const status = String(getValue() ?? '');
      return (
        <Badge
          variant={statusVariant[status] ?? 'outline'}
          className='capitalize'
        >
          <CircleDot className='mr-1 h-3 w-3' />
          {statusLabel[status] ?? status}
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
    accessorKey: 'authorId',
    header: 'Author'
  },
  {
    accessorKey: 'createdAt',
    header: 'Created',
    cell: ({ getValue }) => {
      const raw = getValue() as string | undefined;
      if (!raw) return '—';
      return new Date(raw).toLocaleDateString();
    }
  },
  {
    accessorKey: 'publishedAt',
    header: 'Published',
    cell: ({ getValue }) => {
      const raw = getValue() as string | null | undefined;
      if (!raw) return '—';
      return new Date(raw).toLocaleDateString();
    }
  },
  {
    id: 'actions',
    cell: ({ row }) => <CellAction data={row.original} />
  }
];
