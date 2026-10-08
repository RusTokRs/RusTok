import type { BlogPostStatus, GqlOpts, PostSummary } from '../api/posts';
import { listPosts } from '../api/posts';
import { PostTable, type PostTablePager } from '../components/post-table';
import { columns } from '../components/post-table/columns';

interface PostsPageProps {
  searchParams: {
    /** Opaque cursor of the current page. Absent on the first page. */
    after?: string;
    /** Comma-separated start cursors of the pages before the previous one. */
    history?: string;
    perPage?: string;
    title?: string;
    status?: string;
  };
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
}

function buildPageHref(params: {
  perPage: number;
  status?: BlogPostStatus;
  after?: string;
  history: string[];
}): string {
  const query = new URLSearchParams();
  query.set('perPage', String(params.perPage));
  if (params.status) query.set('status', params.status);
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
  const perPage = Number(searchParams.perPage) || 20;
  const status = searchParams.status as BlogPostStatus | undefined;
  const after = searchParams.after || undefined;
  const history = searchParams.history
    ? searchParams.history.split(',').filter(Boolean)
    : [];

  const opts: GqlOpts = { token, tenantSlug, tenantId };
  const data = await listPosts({ after, perPage, status }, opts);

  const posts: PostSummary[] = data.items;

  // Going back pops the last start cursor from the history; the first page has no cursor.
  const pager: PostTablePager = {
    previousHref:
      after === undefined
        ? null
        : buildPageHref({
            perPage,
            status,
            after: history[history.length - 1],
            history: history.slice(0, -1)
          }),
    nextHref: data.nextCursor
      ? buildPageHref({
          perPage,
          status,
          after: data.nextCursor,
          history: after === undefined ? history : [...history, after]
        })
      : null
  };

  return <PostTable data={posts} pager={pager} columns={columns} />;
}
