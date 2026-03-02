import type { PostSummary } from '../api/posts';
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
}

export default async function PostsPage({
  searchParams,
  token,
  tenantSlug
}: PostsPageProps) {
  const page = Number(searchParams.page) || 1;
  const perPage = Number(searchParams.perPage) || 20;
  const search = searchParams.title || undefined;
  const status = searchParams.status as 'Draft' | 'Published' | 'Archived' | undefined;

  const data = await listPosts(
    { page, per_page: perPage, search, status },
    { token, tenantSlug }
  );

  const posts: PostSummary[] = data.items;

  return <PostTable data={posts} totalItems={data.total} columns={columns} />;
}
