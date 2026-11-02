'use client';

import Link from 'next/link';
import { DataTableShell } from '@/widgets/data-table/data-table-shell';
import { PostSortSelect, PostStatusSelect, PostTextFilter } from './sort-select';
import { createColumns } from './columns';
import type { GqlOpts, PostSummary } from '../../api/posts';
import { useMemo } from 'react';

export interface PostTablePager {
  previousHref: string | null;
  nextHref: string | null;
}

interface PostTableParams {
  data: PostSummary[];
  pager: PostTablePager;
  gqlOpts: GqlOpts;
}

export function PostTable({ data, pager, gqlOpts }: PostTableParams) {
  const { tenantId, tenantSlug, token } = gqlOpts;
  const columns = useMemo(
    () => createColumns({ tenantId, tenantSlug, token }),
    [tenantId, tenantSlug, token]
  );
  return (
    <DataTableShell
      data={data}
      columns={columns}
      pagination={<CursorPagination pager={pager} />}
    >
      <PostTextFilter
        param='title'
        label='Filter by title'
        placeholder='Search title...'
      />
      <PostTextFilter
        param='slug'
        label='Filter by slug'
        placeholder='Search slug...'
      />
      <PostStatusSelect />
      <PostTextFilter
        param='locale'
        label='Filter by locale'
        placeholder='Filter locale...'
      />
      <PostTextFilter
        param='authorId'
        label='Filter by author'
        placeholder='Author id...'
      />
      <PostSortSelect />
    </DataTableShell>
  );
}

function CursorPagination({ pager }: { pager: PostTablePager }) {
  return (
    <nav
      aria-label='Posts pages'
      className='flex items-center justify-end gap-2'
    >
      <PagerLink href={pager.previousHref} label='Previous' />
      <PagerLink href={pager.nextHref} label='Next' />
    </nav>
  );
}

function PagerLink({ href, label }: { href: string | null; label: string }) {
  if (!href) {
    return (
      <span className='text-muted-foreground px-3 py-1.5 text-sm opacity-50'>
        {label}
      </span>
    );
  }
  return (
    <Link
      href={href}
      className='hover:bg-muted rounded-md border px-3 py-1.5 text-sm'
    >
      {label}
    </Link>
  );
}
