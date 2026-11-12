import type {
  BlogPostSortField,
  BlogPostSortOrder,
  BlogPostStatus,
  GqlOpts,
  PostSummary
} from '../api/posts';
import {
  listPosts,
  parsePostSortField,
  parsePostSortOrder
} from '../api/posts';
import { PostTable, type PostTablePager } from '../components/post-table';

interface PostsPageProps {
  searchParams: {
    /** Opaque cursor of the current page. Absent on the first page. */
    after?: string;
    /** Comma-separated start cursors of the pages before the previous one. */
    history?: string;
    perPage?: string;
    /** Case-insensitive substring filter on the locale-resolved title. */
    title?: string;
    /** Case-insensitive substring filter on the post slug. */
    slug?: string;
    status?: string;
    /** Locale filter; narrowed to a BCP-47-shaped tag before it reaches the owner. */
    locale?: string;
    /** Author filter; the wire value is the author's UUID. */
    authorId?: string;
    /** Sort column of the keyset order; narrowed before it reaches the owner. */
    sortBy?: string;
    /** Sort direction of the keyset order; narrowed before it reaches the owner. */
    sortOrder?: string;
  };
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
}

/**
 * Locale filter guard: the owner normalizes and validates locale tags, so an
 * untrusted route value that cannot be a BCP-47-shaped tag is dropped instead
 * of turning into a failed list request.
 */
function parsePostLocaleFilter(value: string | undefined): string | undefined {
  const trimmed = value?.trim();
  if (!trimmed) return undefined;
  return /^[A-Za-z]{2,3}(-[A-Za-z0-9]{1,8})*$/.test(trimmed)
    ? trimmed
    : undefined;
}

/**
 * Author filter guard: the wire value is the author's UUID, so a value that
 * cannot be one can never match and is dropped instead of failing the request.
 */
function parsePostAuthorFilter(value: string | undefined): string | undefined {
  const trimmed = value?.trim();
  if (!trimmed) return undefined;
  return /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/.test(
    trimmed
  )
    ? trimmed.toLowerCase()
    : undefined;
}

function buildPageHref(params: {
  perPage: number;
  title?: string;
  slug?: string;
  status?: BlogPostStatus;
  locale?: string;
  authorId?: string;
  sortBy: BlogPostSortField;
  sortOrder: BlogPostSortOrder;
  after?: string;
  history: string[];
}): string {
  const query = new URLSearchParams();
  query.set('perPage', String(params.perPage));
  // Paging must keep every list-affecting parameter: the cursor in `after` is
  // only valid for the filter and sort that issued it, so a page link that
  // dropped an active filter would resume at a wrong position.
  if (params.title) query.set('title', params.title);
  if (params.slug) query.set('slug', params.slug);
  if (params.status) query.set('status', params.status);
  if (params.locale) query.set('locale', params.locale);
  if (params.authorId) query.set('authorId', params.authorId);
  // Paging must keep the sort: the cursor in `after` is only valid for the sort that issued it.
  query.set('sortBy', params.sortBy);
  query.set('sortOrder', params.sortOrder);
  if (params.after) query.set('after', params.after);
  if (params.history.length > 0) query.set('history', params.history.join(','));
  return `?${query.toString()}`;
}

export default async function PostsPage({
  searchParams,
  token,
  tenantSlug,
  tenantId
}: PostsPageProps) {
  const requestedPageSize = Number(searchParams.perPage);
  const perPage =
    Number.isInteger(requestedPageSize) && requestedPageSize > 0
      ? Math.min(requestedPageSize, 100)
      : 20;
  const status: BlogPostStatus | undefined =
    searchParams.status === 'DRAFT' ||
    searchParams.status === 'PUBLISHED' ||
    searchParams.status === 'ARCHIVED'
      ? searchParams.status
      : undefined;
  const title = searchParams.title?.trim() || undefined;
  const slug = searchParams.slug?.trim() || undefined;
  const locale = parsePostLocaleFilter(searchParams.locale);
  const authorId = parsePostAuthorFilter(searchParams.authorId);
  const after = searchParams.after || undefined;
  const history = searchParams.history
    ? searchParams.history.split(',').filter(Boolean)
    : [];
  const sortBy = parsePostSortField(searchParams.sortBy);
  const sortOrder = parsePostSortOrder(searchParams.sortOrder);

  const opts: GqlOpts = { token, tenantSlug, tenantId };
  const data = await listPosts(
    {
      after,
      perPage,
      status,
      title,
      slug,
      locale,
      authorId,
      sortBy,
      sortOrder
    },
    opts
  );

  const posts: PostSummary[] = data.items;

  // Going back pops the last start cursor from the history; the first page has no cursor.
  const pager: PostTablePager = {
    previousHref:
      after === undefined
        ? null
        : buildPageHref({
            perPage,
            title,
            slug,
            status,
            locale,
            authorId,
            sortBy,
            sortOrder,
            after: history[history.length - 1],
            history: history.slice(0, -1)
          }),
    nextHref: data.nextCursor
      ? buildPageHref({
          perPage,
          title,
          slug,
          status,
          locale,
          authorId,
          sortBy,
          sortOrder,
          after: data.nextCursor,
          history: after === undefined ? history : [...history, after]
        })
      : null
  };

  return <PostTable data={posts} pager={pager} gqlOpts={opts} />;
}
