import type { PostSummary, GqlOpts, BlogPostStatus } from '../api/posts';
import { listPosts } from '../api/posts';
import { PostTable } from '../components/post-table';
import { columns } from '../components/post-table/columns';

interface PostsPageProps {
  searchParams: {
    page?: string;
    perPage?: string;
    title?: string;
    status?: string;
  };
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
}

export default async function PostsPage({
  searchParams,
  token,
  tenantSlug,
  tenantId
}: PostsPageProps) {
  const page = Number(searchParams.page) || 1;
  const perPage = Number(searchParams.perPage) || 20;
  const status = searchParams.status as BlogPostStatus | undefined;

  const opts: GqlOpts = { token, tenantSlug, tenantId };
  const data = await listPosts({ page, perPage, status }, opts);

  const posts: PostSummary[] = data.items;

  return <PostTable data={posts} totalItems={data.total} columns={columns} />;
}
