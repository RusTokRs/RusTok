import {
  BLOG_POST_SORT_FIELDS,
  BLOG_POST_SORT_ORDERS,
  type BlogPostSortField,
  type BlogPostSortOrder
} from '../../api/posts';

// Every `labelKey` is a key of the `blog` message namespace. The keys are literals so that the
// typed translator checks each one against the catalog.
export const STATUS_OPTIONS = [
  { value: 'DRAFT', labelKey: 'status.draft' },
  { value: 'PUBLISHED', labelKey: 'status.published' },
  { value: 'ARCHIVED', labelKey: 'status.archived' }
] as const;

const SORT_FIELD_LABEL_KEYS = {
  CREATED_AT: 'sort.created_at',
  UPDATED_AT: 'sort.updated_at',
  PUBLISHED_AT: 'sort.published_at'
} as const satisfies Record<BlogPostSortField, string>;

const SORT_ORDER_LABEL_KEYS = {
  DESC: 'sort.desc',
  ASC: 'sort.asc'
} as const satisfies Record<BlogPostSortOrder, string>;

/**
 * One option per sort column the owner query indexes.
 *
 * Built from the API-owned list so a column the owner stops indexes cannot linger in the UI, and
 * so the option value is the wire value instead of a label-derived slug.
 */
export const SORT_FIELD_OPTIONS = BLOG_POST_SORT_FIELDS.map((value) => ({
  value,
  labelKey: SORT_FIELD_LABEL_KEYS[value]
}));

/** Sort directions, from the same owner list the GraphQL enum uses. */
export const SORT_ORDER_OPTIONS = BLOG_POST_SORT_ORDERS.map((value) => ({
  value,
  labelKey: SORT_ORDER_LABEL_KEYS[value]
}));
