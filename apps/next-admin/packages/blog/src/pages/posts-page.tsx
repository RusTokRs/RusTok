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
    title?: string;
    status?: string;
    /** Sort column of the keyset order; narrowed before it reaches the owner. */
    sortBy?: string;
    /** Sort direction of the keyset order; narrowed before it reaches the owner. */
    sortOrder?: string;
  };
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
}

function buildPageHref(params: {
  perPage: number;
  status?: BlogPostStatus;
  sortBy: BlogPostSortField;
  sortOrder: BlogPostSortOrder;
  after?: string;
  history: string[];
}): string {
  const query = new URLSearchParams();
  query.set('perPage', String(params.perPage));
  if (params.status) query.set('status', params.status);
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
  const perPage = Number.isInteger(requestedPageSize) && requestedPageSize > 0
    ? Math.min(requestedPageSize, 100)
    : 20;
  const status: BlogPostStatus | undefined =
    searchParams.status === 'DRAFT' ||
    searchParams.status === 'PUBLISHED' ||
    searchParams.status === 'ARCHIVED'
      ? searchParams.status
      : undefined;
  const after = searchParams.after || undefined;
  const history = searchParams.history
    ? searchParams.history.split(',').filter(Boolean)
    : [];
  const sortBy = parsePostSortField(searchParams.sortBy);
  const sortOrder = parsePostSortOrder(searchParams.sortOrder);

  const opts: GqlOpts = { token, tenantSlug, tenantId };
  const data = await listPosts(
    { after, perPage, status, sortBy, sortOrder },
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
            status,
            sortBy,
            sortOrder,
            after: history[history.length - 1],
            history: history.slice(0, -1)
          }),
    nextHref: data.nextCursor
      ? buildPageHref({
          perPage,
          status,
          sortBy,
          sortOrder,
          after: data.nextCursor,
          history: after === undefined ? history : [...history, after]
        })
      : null
  };

  return <PostTable data={posts} pager={pager} gqlOpts={opts} />;
}
