'use client';

import { useState } from 'react';
import { useRouter } from 'next/navigation';
import { MessageSquare } from 'lucide-react';
import type { BlogPublicComment } from '../api/posts';
import { BlogCommentComposer } from './blog-comment-composer';
import { ThreadedCommentsList } from './threaded-comments';

interface BlogCommentsSectionProps {
  tenantId: string;
  tenantSlug: string;
  postId: string;
  contentLocale: string;
  comments: {
    availability: string;
    total: number;
    items: BlogPublicComment[];
  };
  degradedMessage?: string | null;
  locale?: string;
}

export function BlogCommentsSection({
  tenantId,
  tenantSlug,
  postId,
  contentLocale,
  comments,
  degradedMessage,
  locale = 'ru'
}: BlogCommentsSectionProps) {
  const router = useRouter();
  const [replyToId, setReplyToId] = useState<string | null>(null);
  const isRu = locale === 'ru';

  const handleReply = (commentId: string) => {
    setReplyToId(commentId);
    const composerElem = document.getElementById('comment-composer-box');
    if (composerElem) {
      composerElem.scrollIntoView({ behavior: 'smooth', block: 'center' });
    }
  };

  const handleCommentSubmitted = () => {
    setReplyToId(null);
    router.refresh();
  };

  return (
    <section id='comments' className='mt-12 border-t border-border pt-8 space-y-6'>
      <div className='flex items-center justify-between gap-3'>
        <div className='flex items-center gap-2'>
          <MessageSquare className='h-5 w-5 text-primary' />
          <h2 className='text-xl font-bold text-foreground'>
            {isRu ? 'Комментарии и обсуждение' : 'Comments & Discussion'}
          </h2>
        </div>
        {comments.total > 0 && (
          <span className='text-xs text-muted-foreground font-medium'>
            {comments.total}{' '}
            {isRu ? 'комментариев' : 'total comments'}
          </span>
        )}
      </div>

      {degradedMessage && (
        <div className='rounded-xl border border-border bg-muted/40 p-4 text-sm text-muted-foreground'>
          {degradedMessage}
        </div>
      )}

      {comments.availability === 'AVAILABLE' && (
        <div
          id='comment-composer-box'
          className='rounded-2xl border border-border bg-card p-5 shadow-sm'
        >
          <BlogCommentComposer
            tenantId={tenantId}
            tenantSlug={tenantSlug}
            postId={postId}
            contentLocale={contentLocale}
            parentCommentId={replyToId}
            onCancelReply={() => setReplyToId(null)}
            onCommentSubmitted={handleCommentSubmitted}
          />
        </div>
      )}

      <ThreadedCommentsList
        items={comments.items}
        locale={locale}
        onReply={comments.availability === 'AVAILABLE' ? handleReply : undefined}
      />
    </section>
  );
}
