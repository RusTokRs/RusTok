import {
  getFormatter,
  getLocale,
  getTranslations
} from '@rustok/next-fluent/server';
import { getPost } from '../api/posts';
import type { PostResponse, GqlOpts } from '../api/posts';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { RichTextHtml } from '@rustok/richtext/view';
import { postStatusPresentation } from '../model/presentation';

interface PostDetailPageProps {
  postId: string;
  token?: string | null;
  tenantSlug?: string | null;
  tenantId?: string | null;
}

export default async function PostDetailPage({
  postId,
  token,
  tenantSlug,
  tenantId
}: PostDetailPageProps) {
  const locale = await getLocale();
  const t = await getTranslations({ namespace: 'blog', locale });
  const formatter = await getFormatter({ locale });
  const opts: GqlOpts = {
    token,
    tenantSlug,
    tenantId,
    locale
  };
  const post: PostResponse | null = await getPost(postId, opts);

  if (!post) {
    return <p>{t('detail.notFound')}</p>;
  }

  const status = postStatusPresentation(post.status);

  return (
    <Card>
      <CardHeader>
        <div className='flex items-center gap-3'>
          <CardTitle className='text-2xl'>{post.title}</CardTitle>
          <Badge variant={status.variant}>{t(status.labelKey)}</Badge>
        </div>
        <p className='text-muted-foreground text-sm'>
          {post.slug}
          {post.publishedAt && (
            <>
              {' '}
              &middot;{' '}
              {t('detail.publishedAt', {
                date: formatter.dateTime(post.publishedAt, {
                  dateStyle: 'medium'
                })
              })}
            </>
          )}
        </p>
      </CardHeader>
      <CardContent className='space-y-4'>
        {post.excerpt && (
          <p className='text-muted-foreground italic'>{post.excerpt}</p>
        )}
        {post.content?.html && (
          <RichTextHtml
            view={post.content}
            contentLocale={post.effectiveLocale}
            className='prose max-w-none'
          />
        )}
        {post.tags.length > 0 && (
          <div className='flex flex-wrap gap-2'>
            {post.tags.map((tag) => (
              <Badge key={tag} variant='outline'>
                {tag}
              </Badge>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
