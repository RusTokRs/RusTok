import { getPost } from '../api/posts';
import type { GqlOpts } from '../api/posts';
import PostForm from '../components/post-form';

interface PostFormPageProps {
  postId?: string;
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
}

export default async function PostFormPage({
  postId,
  token,
  tenantSlug,
  tenantId
}: PostFormPageProps) {
  const opts: GqlOpts = { token, tenantSlug, tenantId };
  const initialData = postId ? await getPost(postId, opts) : null;

  return (
    <PostForm
      initialData={initialData}
      pageTitle={initialData ? 'Edit Post' : 'Create Post'}
      gqlOpts={opts}
    />
  );
}
