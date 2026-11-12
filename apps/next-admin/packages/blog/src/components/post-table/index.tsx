'use client';

import { useTranslations } from '@rustok/next-fluent/client';
import Link from 'next/link';
import { DataTableShell } from '@/widgets/data-table/data-table-shell';
import {
  PostSortSelect,
  PostStatusSelect,
  PostTextFilter
} from './sort-select';
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
  const t = useTranslations('blog');
  const { tenantId, tenantSlug, token } = gqlOpts;
  const columns = useMemo(
    () => createColumns({ tenantId, tenantSlug, token }, t),
    [tenantId, tenantSlug, token, t]
  );
  return (
    <DataTableShell
      data={data}
      columns={columns}
      pagination={<CursorPagination pager={pager} />}
    >
      <PostTextFilter
        param='title'
        label={t('table.filterTitle')}
        placeholder={t('table.searchTitle')}
      />
      <PostTextFilter
        param='slug'
        label={t('table.filterSlug')}
        placeholder={t('table.searchSlug')}
      />
      <PostStatusSelect />
      <PostTextFilter
        param='locale'
        label={t('table.filterLocale')}
        placeholder={t('table.searchLocale')}
      />
      <PostTextFilter
        param='authorId'
        label={t('table.filterAuthor')}
        placeholder={t('table.searchAuthor')}
      />
      <PostSortSelect />
    </DataTableShell>
  );
}

function CursorPagination({ pager }: { pager: PostTablePager }) {
  const t = useTranslations('blog');
  return (
    <nav
      aria-label={t('table.pages')}
      className='flex items-center justify-end gap-2'
    >
      <PagerLink href={pager.previousHref} label={t('pagination.previous')} />
      <PagerLink href={pager.nextHref} label={t('pagination.next')} />
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
