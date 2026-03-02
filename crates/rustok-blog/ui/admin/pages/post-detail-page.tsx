import { getPost } from '../api/posts';
import type { PostResponse } from '../api/posts';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';

interface PostDetailPageProps {
  postId: string;
  locale?: string;
  token?: string | null;
  tenantSlug?: string | null;
}

const statusVariant: Record<string, 'default' | 'secondary' | 'outline'> = {
  Published: 'default',
  Draft: 'secondary',
  Archived: 'outline'
};

export default async function PostDetailPage({
  postId,
  locale = 'en',
  token,
  tenantSlug
}: PostDetailPageProps) {
  const post: PostResponse = await getPost(postId, locale, { token, tenantSlug });

  return (
    <Card>
      <CardHeader>
        <div className='flex items-center gap-3'>
          <CardTitle className='text-2xl'>{post.title}</CardTitle>
          <Badge variant={statusVariant[post.status] ?? 'outline'}>
            {post.status}
          </Badge>
        </div>
        <p className='text-muted-foreground text-sm'>
          {post.slug} &middot; {post.locale}
          {post.published_at && (
            <> &middot; Published {new Date(post.published_at).toLocaleDateString()}</>
          )}
        </p>
      </CardHeader>
      <CardContent className='space-y-4'>
        {post.excerpt && (
          <p className='text-muted-foreground italic'>{post.excerpt}</p>
        )}
        <div className='prose max-w-none whitespace-pre-wrap'>
          {post.body}
        </div>
        {post.tags.length > 0 && (
          <div className='flex flex-wrap gap-2'>
            {post.tags.map((tag) => (
              <Badge key={tag} variant='outline'>{tag}</Badge>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
