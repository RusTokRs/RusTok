'use client';

import { useTranslations } from '@rustok/next-fluent/client';
import { useEffect, useState } from 'react';
import { parseAsString, parseAsStringLiteral, useQueryStates } from 'nuqs';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
import { Input } from '@/shared/ui/shadcn/input';
import {
  BLOG_POST_SORT_FIELDS,
  BLOG_POST_SORT_ORDERS,
  DEFAULT_POST_SORT_FIELD,
  DEFAULT_POST_SORT_ORDER,
  parsePostSortField,
  parsePostSortOrder
} from '../../api/posts';
import { SORT_FIELD_OPTIONS, SORT_ORDER_OPTIONS, STATUS_OPTIONS } from './options';

/**
 * Sort control for the cursor-paged post list.
 *
 * The sort is part of the list request rather than a client-side reorder, and the owner refuses a
 * cursor that was issued for a different sort. So changing either half of the sort drops `after`
 * and the page history: the list restarts at page one with the new order instead of resuming a
 * stale position. The shell re-renders on the server because the update is not shallow.
 */
export function PostSortSelect() {
  const t = useTranslations('blog');
  const [sort, setSort] = useQueryStates(
    {
      sortBy: parseAsStringLiteral(BLOG_POST_SORT_FIELDS).withDefault(
        DEFAULT_POST_SORT_FIELD
      ),
      sortOrder: parseAsStringLiteral(BLOG_POST_SORT_ORDERS).withDefault(
        DEFAULT_POST_SORT_ORDER
      ),
      after: parseAsString,
      history: parseAsString
    },
    { shallow: false, history: 'push' }
  );

  return (
    <div className='flex items-center gap-2'>
      <Select
        value={sort.sortBy}
        onValueChange={(value) =>
          void setSort({
            sortBy: parsePostSortField(value),
            after: null,
            history: null
          })
        }
      >
        <SelectTrigger className='h-8 w-[170px]' aria-label={t('table.sortField')}>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {SORT_FIELD_OPTIONS.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {t(`sort.${option.value.toLowerCase()}`)}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      <Select
        value={sort.sortOrder}
        onValueChange={(value) =>
          void setSort({
            sortOrder: parsePostSortOrder(value),
            after: null,
            history: null
          })
        }
      >
        <SelectTrigger className='h-8 w-[140px]' aria-label={t('table.sortOrder')}>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {SORT_ORDER_OPTIONS.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {t(`sort.${option.value.toLowerCase()}`)}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}

/** Blog owner accepts exactly one post status. Changing it invalidates keyset cursors. */
export function PostStatusSelect() {
  const t = useTranslations('blog');
  const [filters, setFilters] = useQueryStates(
    {
      status: parseAsStringLiteral(['DRAFT', 'PUBLISHED', 'ARCHIVED'] as const),
      after: parseAsString,
      history: parseAsString
    },
    { shallow: false, history: 'push' }
  );

  return (
    <Select
      value={filters.status ?? 'ALL'}
      onValueChange={(value) =>
        void setFilters({
          status: value === 'ALL' ? null : value as 'DRAFT' | 'PUBLISHED' | 'ARCHIVED',
          after: null,
          history: null
        })
      }
    >
      <SelectTrigger className='h-8 w-[150px]' aria-label={t('table.filterStatus')}>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value='ALL'>{t('table.allStatuses')}</SelectItem>
        {STATUS_OPTIONS.map((option) => (
          <SelectItem key={option.value} value={option.value}>
            {t(`status.${option.value.toLowerCase()}`)}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

/**
 * Text filters for the cursor-paged post list: title, slug, locale and author.
 *
 * Each filter is part of the list request (the owner filters server-side), so a
 * change invalidates the keyset cursor exactly like a sort or status change
 * does: the update resets `after` and the page history atomically, and the list
 * restarts at page one under the new filter instead of resuming a stale
 * position that would silently skip matching rows.
 */
const POST_TEXT_FILTER_PARAMS = ['title', 'slug', 'locale', 'authorId'] as const;

type PostTextFilterParam = (typeof POST_TEXT_FILTER_PARAMS)[number];

function postTextFilterParsers(param: PostTextFilterParam) {
  const parsers: Record<
    PostTextFilterParam | 'after' | 'history',
    typeof parseAsString
  > = {
    title: parseAsString,
    slug: parseAsString,
    locale: parseAsString,
    authorId: parseAsString,
    after: parseAsString,
    history: parseAsString
  };
  // Only the active param plus the cursor pair are wired into the query state;
  // the other text filters keep their own URL keys untouched.
  return {
    [param]: parsers[param],
    after: parsers.after,
    history: parsers.history
  } as Pick<typeof parsers, PostTextFilterParam | 'after' | 'history'>;
}

export function PostTextFilter({
  param,
  label,
  placeholder
}: {
  param: PostTextFilterParam;
  label: string;
  placeholder: string;
}) {
  const [filters, setFilters] = useQueryStates(
    postTextFilterParsers(param),
    { shallow: false, history: 'push' }
  );

  const value = filters[param] ?? '';

  // The input is edited locally; the route (and with it the server list)
  // updates only after typing pauses, so one search phrase is one navigation
  // instead of one per keystroke.
  const [draft, setDraft] = useState(value);
  useEffect(() => {
    setDraft(value);
  }, [value]);
  useEffect(() => {
    if (draft === value) return;
    const timer = setTimeout(() => {
      void setFilters({
        [param]: draft.trim() === '' ? null : draft,
        after: null,
        history: null
      });
    }, 400);
    return () => clearTimeout(timer);
  }, [draft, value, param, setFilters]);

  return (
    <Input
      type='search'
      className='h-8 w-[170px]'
      aria-label={label}
      placeholder={placeholder}
      maxLength={255}
      value={draft}
      onChange={(event) => setDraft(event.target.value)}
    />
  );
}
