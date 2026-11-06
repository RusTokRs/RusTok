'use client';

import { Badge } from '@/components/ui/badge';
import { DataTableColumnHeader } from '@/components/ui/table/data-table-column-header';
import type { ColumnDef, StockFeatures } from '@tanstack/react-table';
import { CircleDot } from 'lucide-react';
import { CellAction } from './cell-action';
import type { GqlOpts, PostSummary } from '../../api/posts';
import { postStatusPresentation } from '../../model/presentation';

/**
 * ISO `YYYY-MM-DD` date cell: a deterministic slice of the owner's RFC 3339
 * timestamp, stable across server and client locales.
 */
function dateCell(raw: string | null | undefined): string {
  return raw ? raw.slice(0, 10) : '—';
}

/**
 * Author cell: the owner's profile-enriched display name, the raw author id
 * when no profile is available, and an em dash only for a missing author.
 */
function authorCell(post: PostSummary): string {
  const name = post.authorProfile?.displayName?.trim();
  if (name) return name;
  return post.authorId ?? '—';
}

/**
 * Column set of the admin post list, mirroring the Leptos admin grid:
 * title (with excerpt), slug, status, locale, author, created, published,
 * actions.
 *
 * Sorting is owned by `PostSortSelect` (the sort is part of the keyset request,
 * not a client-side reorder), so no column enables the client sort menu;
 * filtering is owned by the text-filter controls next to it, which reset the
 * cursor atomically when a filter changes.
 */
export function createColumns(gqlOpts: GqlOpts, t: (key: string) => string): ColumnDef<StockFeatures, PostSummary, any>[] {
  return [
  {
    id: 'title',
    accessorKey: 'title',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title={t('grid.title')} />
    ),
    cell: ({ row }) => (
      <div className='max-w-[300px]'>
        <div className='truncate font-medium'>{row.original.title}</div>
        {row.original.excerpt ? (
          <div className='truncate text-xs text-muted-foreground'>
            {row.original.excerpt}
          </div>
        ) : null}
      </div>
    ),
    enableSorting: false,
    meta: {
      label: t('grid.title')
    }
  },
  {
    id: 'slug',
    accessorKey: 'slug',
    header: t('grid.slug'),
    cell: ({ getValue }) => (
      <span className='text-muted-foreground truncate'>
        {String(getValue() ?? '—')}
      </span>
    ),
    enableSorting: false,
    meta: {
      label: t('grid.slug')
    }
  },
  {
    id: 'status',
    accessorKey: 'status',
    header: ({ column }) => (
      <DataTableColumnHeader column={column} title={t('grid.status')} />
    ),
    cell: ({ getValue }) => {
      const status = String(getValue() ?? '');
      const presentation = postStatusPresentation(status);
      return (
        <Badge
          variant={presentation.variant}
          className='capitalize'
        >
          <CircleDot className='mr-1 h-3 w-3' />
          {t(presentation.labelKey)}
        </Badge>
      );
    },
    enableSorting: false,
    meta: {
      label: t('grid.status')
    }
  },
  {
    id: 'locale',
    accessorKey: 'effectiveLocale',
    header: t('grid.locale'),
    cell: ({ getValue }) => (
      <span className='text-muted-foreground'>
        {String(getValue() ?? '')}
      </span>
    ),
    enableSorting: false,
    meta: {
      label: t('grid.locale')
    }
  },
  {
    id: 'author',
    accessorKey: 'authorId',
    header: t('grid.author'),
    cell: ({ row }) => (
      <span className='text-muted-foreground truncate'>
        {authorCell(row.original)}
      </span>
    ),
    enableSorting: false,
    meta: {
      label: t('grid.author')
    }
  },
  {
    id: 'createdAt',
    accessorKey: 'createdAt',
    header: t('grid.created'),
    cell: ({ row }) => (
      <span className='text-muted-foreground'>
        {dateCell(row.original.createdAt)}
      </span>
    ),
    enableSorting: false,
    meta: {
      label: t('grid.created')
    }
  },
  {
    id: 'publishedAt',
    accessorKey: 'publishedAt',
    header: t('grid.published'),
    cell: ({ row }) => (
      <span className='text-muted-foreground'>
        {dateCell(row.original.publishedAt)}
      </span>
    ),
    enableSorting: false,
    meta: {
      label: t('grid.published')
    }
  },
  {
    id: 'actions',
    cell: ({ row }) => <CellAction data={row.original} gqlOpts={gqlOpts} />
  }
];

}
