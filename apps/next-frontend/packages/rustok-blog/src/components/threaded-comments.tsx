'use client';

import { useMemo } from 'react';
import { Reply, User } from 'lucide-react';
import type { BlogPublicComment } from '../api/posts';
import { orderThreadedComments } from '../utils/comments-order';

interface ThreadedCommentsProps {
  items: BlogPublicComment[];
  locale?: string;
  onReply?: (parentCommentId: string) => void;
}

export function ThreadedCommentsList({
  items,
  locale = 'ru',
  onReply
}: ThreadedCommentsProps) {
  const isRu = locale.startsWith('ru');
  const orderedComments = useMemo(() => orderThreadedComments(items), [items]);

  if (items.length === 0) {
    return (
      <div className='rounded-xl border border-dashed border-border p-6 text-center text-sm text-muted-foreground'>
        {isRu
          ? 'Пока нет комментариев. Оставьте отзыв первым!'
          : 'No comments yet. Be the first to share your thoughts!'}
      </div>
    );
  }

  return (
    <div className='space-y-3'>
      {orderedComments.map(({ comment, depth }) => (
        <article
          key={comment.id}
          className='rounded-xl border border-border bg-card p-4 space-y-2 shadow-sm'
          style={{ marginLeft: Math.min(depth, 3) * 16 }}
        >
          <div className='flex items-center justify-between gap-2 text-xs text-muted-foreground'>
            <div className='flex items-center gap-1.5 font-medium text-foreground/80'>
              <User className='h-3.5 w-3.5 text-muted-foreground' />
              <span>{depth > 0
                ? isRu ? 'Ответ' : 'Reply'
                : isRu ? 'Пользователь' : 'User'}</span>
            </div>
            <time dateTime={comment.createdAt}>
              {new Date(comment.createdAt).toLocaleDateString(locale, {
                year: 'numeric',
                month: 'short',
                day: 'numeric'
              })}
            </time>
          </div>
          <p className='whitespace-pre-line text-sm leading-6 text-foreground'>
            {comment.contentPreview}
          </p>
          {onReply && (
            <div className='pt-1'>
              <button
                type='button'
                onClick={() => onReply(comment.id)}
                className='inline-flex items-center gap-1 text-xs font-medium text-primary hover:underline'
              >
                <Reply className='h-3.5 w-3.5' />
                <span>{isRu ? 'Ответить' : 'Reply'}</span>
              </button>
            </div>
          )}
        </article>
      ))}
    </div>
  );
}
