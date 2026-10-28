'use client';

import { parseAsString, parseAsStringLiteral, useQueryStates } from 'nuqs';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from '@/shared/ui/shadcn/select';
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
        <SelectTrigger className='h-8 w-[170px]' aria-label='Sort by'>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {SORT_FIELD_OPTIONS.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {option.label}
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
        <SelectTrigger className='h-8 w-[140px]' aria-label='Sort order'>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {SORT_ORDER_OPTIONS.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}

/** Blog owner accepts exactly one post status. Changing it invalidates keyset cursors. */
export function PostStatusSelect() {
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
      <SelectTrigger className='h-8 w-[150px]' aria-label='Filter by status'>
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value='ALL'>All statuses</SelectItem>
        {STATUS_OPTIONS.map((option) => (
          <SelectItem key={option.value} value={option.value}>
            {option.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
