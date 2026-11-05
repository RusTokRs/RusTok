import { getLocale, getTranslations } from '@rustok/next-fluent/server';
import { notFound } from 'next/navigation';
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
  const locale = await getLocale();
  const t = await getTranslations('blog');
  const opts: GqlOpts = { token, tenantSlug, tenantId, locale };
  const initialData = postId ? await getPost(postId, opts) : null;

  if (postId && !initialData) notFound();
  return (
    <PostForm
      initialData={initialData}
      pageTitle={initialData ? t('form.editTitle') : t('form.createTitle')}
      gqlOpts={opts}
    />
  );
}
