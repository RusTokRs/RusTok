'use client';

import { CommentComposer } from '@rustok/comments-frontend';
import type { RichTextDocument } from '@rustok/richtext';
import { useTranslations } from '@rustok/next-fluent';
import { useEffect, useState } from 'react';

import { getClientAuth, type AuthSession } from '@/shared/lib/auth';
import { storefrontGraphql } from '@/shared/lib/graphql';
import { createBlogComment } from '../api/posts';

import { X } from 'lucide-react';

export function BlogCommentComposer({
  tenantId,
  tenantSlug,
  postId,
  contentLocale,
  parentCommentId,
  onCancelReply,
  onCommentSubmitted
}: {
  tenantId: string;
  tenantSlug: string;
  postId: string;
  contentLocale: string;
  parentCommentId?: string | null;
  onCancelReply?: () => void;
  onCommentSubmitted?: () => void;
}) {
  const t = useTranslations('Comments.composer');
  const [auth, setAuth] = useState<AuthSession | null>(null);
  useEffect(() => setAuth(getClientAuth()), []);

  async function submit(content: RichTextDocument, commandId: string) {
    if (!auth?.token) throw new Error(t('signInRequired'));
    await createBlogComment(
      storefrontGraphql,
      tenantId,
      tenantSlug,
      auth.token,
      postId,
      contentLocale,
      content,
      commandId,
      parentCommentId
    );
    if (onCommentSubmitted) {
      onCommentSubmitted();
    }
  }

  return (
    <div className='space-y-3'>
      {parentCommentId && (
        <div className='flex items-center justify-between rounded-lg bg-muted/60 px-3 py-1.5 text-xs text-muted-foreground'>
          <span>Replying to comment</span>
          {onCancelReply && (
            <button
              type='button'
              onClick={onCancelReply}
              className='inline-flex items-center gap-1 text-xs hover:text-foreground'
            >
              <X className='h-3 w-3' /> Cancel
            </button>
          )}
        </div>
      )}
      <CommentComposer
        contentLocale={contentLocale}
        canSubmit={Boolean(auth?.token)}
        onSubmit={submit}
      />
    </div>
  );
}
