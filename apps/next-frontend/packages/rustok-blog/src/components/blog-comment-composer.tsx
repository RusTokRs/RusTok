'use client';

import { CommentComposer } from '@rustok/comments-frontend';
import type { RichTextDocument } from '@rustok/richtext';
import { useTranslations } from 'next-intl';
import { useEffect, useState } from 'react';

import { getClientAuth, type AuthSession } from '@/shared/lib/auth';
import { storefrontGraphql } from '@/shared/lib/graphql';
import { createBlogComment } from '../api/posts';

export function BlogCommentComposer({
  tenantId,
  tenantSlug,
  postId,
  contentLocale
}: {
  tenantId: string;
  tenantSlug: string;
  postId: string;
  contentLocale: string;
}) {
  const t = useTranslations('Comments.composer');
  const [auth, setAuth] = useState<AuthSession | null>(null);
  useEffect(() => setAuth(getClientAuth()), []);

  async function submit(content: RichTextDocument) {
    if (!auth?.token) throw new Error(t('signInRequired'));
    await createBlogComment(
      storefrontGraphql,
      tenantId,
      tenantSlug,
      auth.token,
      postId,
      contentLocale,
      content
    );
  }

  return (
    <CommentComposer
      contentLocale={contentLocale}
      canSubmit={Boolean(auth?.token)}
      onSubmit={submit}
    />
  );
}
