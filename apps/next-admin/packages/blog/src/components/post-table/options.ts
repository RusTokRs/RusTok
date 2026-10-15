import {
  BLOG_POST_SORT_FIELDS,
  BLOG_POST_SORT_ORDERS,
  type BlogPostSortField,
  type BlogPostSortOrder
} from '../../api/posts';

export const STATUS_OPTIONS = [
  { value: 'DRAFT', label: 'Draft' },
  { value: 'PUBLISHED', label: 'Published' },
  { value: 'ARCHIVED', label: 'Archived' }
];

const SORT_FIELD_LABELS: Record<BlogPostSortField, string> = {
  CREATED_AT: 'Newest created',
  UPDATED_AT: 'Recently updated',
  PUBLISHED_AT: 'Recently published'
};

const SORT_ORDER_LABELS: Record<BlogPostSortOrder, string> = {
  DESC: 'Descending',
  ASC: 'Ascending'
};

/**
 * One option per sort column the owner query indexes.
 *
 * Built from the API-owned list so a column the owner stops indexes cannot linger in the UI, and
 * so the option value is the wire value instead of a label-derived slug.
 */
export const SORT_FIELD_OPTIONS: { value: BlogPostSortField; label: string }[] =
  BLOG_POST_SORT_FIELDS.map((value) => ({
    value,
    label: SORT_FIELD_LABELS[value]
  }));

/** Sort directions, from the same owner list the GraphQL enum uses. */
export const SORT_ORDER_OPTIONS: { value: BlogPostSortOrder; label: string }[] =
  BLOG_POST_SORT_ORDERS.map((value) => ({
    value,
    label: SORT_ORDER_LABELS[value]
  }));
