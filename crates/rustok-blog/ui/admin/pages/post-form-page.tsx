import { getPost } from '../api/posts';
import PostForm from '../components/post-form';

interface PostFormPageProps {
  postId?: string;
  token?: string | null;
  tenantSlug?: string | null;
}

export default async function PostFormPage({
  postId,
  token,
  tenantSlug
}: PostFormPageProps) {
  const initialData = postId
    ? await getPost(postId, 'en', { token, tenantSlug })
    : null;

  return (
    <PostForm
      initialData={initialData}
      pageTitle={initialData ? 'Edit Post' : 'Create Post'}
    />
  );
}
