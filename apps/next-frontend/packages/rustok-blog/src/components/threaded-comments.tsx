'use client';

import { useMemo } from 'react';
import { Reply, User } from 'lucide-react';
import type { BlogPublicComment } from '../api/posts';

interface ThreadedCommentsProps {
  items: BlogPublicComment[];
  locale?: string;
  onReply?: (parentCommentId: string) => void;
}

interface CommentNode {
  comment: BlogPublicComment;
  replies: BlogPublicComment[];
}

export function ThreadedCommentsList({
  items,
  locale = 'ru',
  onReply
}: ThreadedCommentsProps) {
  const isRu = locale === 'ru';

  const { rootComments, repliesMap } = useMemo(() => {
    const replies = new Map<string, BlogPublicComment[]>();
    const roots: BlogPublicComment[] = [];

    for (const item of items) {
      if (item.parentCommentId) {
        const existing = replies.get(item.parentCommentId) ?? [];
        existing.push(item);
        replies.set(item.parentCommentId, existing);
      } else {
        roots.push(item);
      }
    }

    return { rootComments: roots, repliesMap: replies };
  }, [items]);

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
    <div className='space-y-4'>
      {rootComments.map((root) => {
        const replies = repliesMap.get(root.id) ?? [];
        return (
          <article
            key={root.id}
            className='rounded-xl border border-border bg-card p-4 space-y-2 shadow-sm'
          >
            <div className='flex items-center justify-between text-xs text-muted-foreground'>
              <div className='flex items-center gap-1.5 font-medium text-foreground/80'>
                <User className='h-3.5 w-3.5 text-muted-foreground' />
                <span>{isRu ? 'Пользователь' : 'User'}</span>
              </div>
              <time dateTime={root.createdAt}>
                {new Date(root.createdAt).toLocaleDateString(locale, {
                  year: 'numeric',
                  month: 'short',
                  day: 'numeric'
                })}
              </time>
            </div>

            <p className='whitespace-pre-line text-sm leading-6 text-foreground'>
              {root.contentPreview}
            </p>

            {onReply && (
              <div className='pt-1'>
                <button
                  type='button'
                  onClick={() => onReply(root.id)}
                  className='inline-flex items-center gap-1 text-xs font-medium text-primary hover:underline'
                >
                  <Reply className='h-3.5 w-3.5' />
                  <span>{isRu ? 'Ответить' : 'Reply'}</span>
                </button>
              </div>
            )}

            {/* Nested replies */}
            {replies.length > 0 && (
              <div className='mt-3 ml-4 pl-4 border-l-2 border-border/80 space-y-3'>
                {replies.map((reply) => (
                  <div
                    key={reply.id}
                    className='rounded-lg bg-muted/40 p-3 space-y-1'
                  >
                    <div className='flex items-center justify-between text-[11px] text-muted-foreground'>
                      <div className='flex items-center gap-1 font-medium text-foreground/80'>
                        <User className='h-3 w-3 text-muted-foreground' />
                        <span>{isRu ? 'Ответ' : 'Reply'}</span>
                      </div>
                      <time dateTime={reply.createdAt}>
                        {new Date(reply.createdAt).toLocaleDateString(locale, {
                          year: 'numeric',
                          month: 'short',
                          day: 'numeric'
                        })}
                      </time>
                    </div>
                    <p className='whitespace-pre-line text-xs leading-5 text-foreground'>
                      {reply.contentPreview}
                    </p>
                  </div>
                ))}
              </div>
            )}
          </article>
        );
      })}
    </div>
  );
}
