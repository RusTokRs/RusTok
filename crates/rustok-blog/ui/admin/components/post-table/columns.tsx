'use client';

import { Badge } from '@/components/ui/badge';
import { DataTableColumnHeader } from '@/components/ui/table/data-table-column-header';
import { Column, ColumnDef } from '@tanstack/react-table';
import { Text, CircleDot } from 'lucide-react';
import { CellAction } from './cell-action';
import { STATUS_OPTIONS } from './options';
import type { PostSummary } from '../../api/posts';

const statusVariant: Record<string, 'default' | 'secondary' | 'outline'> = {
  Published: 'default',
  Draft: 'secondary',
  Archived: 'outline'
};

export const columns: ColumnDef<PostSummary>[] = [
  {
    id: 'title',
    accessorKey: 'title',
    header: ({ column }: { column: Column<PostSummary, unknown> }) => (
      <DataTableColumnHeader column={column} title='Title' />
    ),
    cell: ({ cell }) => (
      <div className='max-w-[300px] truncate font-medium'>
        {cell.getValue<string>()}
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
    header: ({ column }: { column: Column<PostSummary, unknown> }) => (
      <DataTableColumnHeader column={column} title='Status' />
    ),
    cell: ({ cell }) => {
      const status = cell.getValue<string>();
      return (
        <Badge variant={statusVariant[status] ?? 'outline'} className='capitalize'>
          <CircleDot className='mr-1 h-3 w-3' />
          {status}
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
    accessorKey: 'author_name',
    header: 'Author'
  },
  {
    accessorKey: 'category_name',
    header: 'Category'
  },
  {
    accessorKey: 'created_at',
    header: 'Created',
    cell: ({ cell }) => {
      const raw = cell.getValue<string>();
      if (!raw) return '—';
      return new Date(raw).toLocaleDateString();
    }
  },
  {
    accessorKey: 'published_at',
    header: 'Published',
    cell: ({ cell }) => {
      const raw = cell.getValue<string | null>();
      if (!raw) return '—';
      return new Date(raw).toLocaleDateString();
    }
  },
  {
    id: 'actions',
    cell: ({ row }) => <CellAction data={row.original} />
  }
];
