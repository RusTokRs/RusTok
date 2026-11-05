'use client';
import { Button } from '@/shared/ui/base/button';

/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import { useTranslations } from '@rustok/next-fluent';
import { useCallback, useEffect, useState } from 'react';
import Image from 'next/image';
import { Calendar, Clock, Eye, Tag } from 'lucide-react';
import { storefrontGraphql } from '@/shared/lib/graphql';
import { RichTextHtml } from '@rustok/richtext/view';
import {
  fetchPostByPreviewToken,
  type BlogPreviewPostDetail,
} from '../api/posts';
import { AuthorMiniBadge } from './author-card';
import { calculateReadingTime, formatReadingTime } from '../utils/reading-time';

/**
 * Draft preview surface for Blog preview-token links.
 *
 * The token arrives in the URL *fragment* (`/blog/preview#token=…`) so it is
 * never sent to the server in a request, never logged by proxies and never
 * leaks through a referrer. Fragments are only readable in the browser, which
 * is why this island is a client component: it parses the hash on mount (and
 * on hash change), resolves the draft through the unauthenticated
 * `postByPreviewToken` query, and offers a paste fallback when the fragment
 * was stripped or the reviewer only received the raw token.
 */

interface BlogDraftPreviewProps {
  locale: string;
  tenantSlug: string | null;
}

function tokenFromHash(hash: string): string | null {
  const raw = hash.replace(/^#/, '');
  if (!raw) return null;
  const parsed = new URLSearchParams(raw);
  return parsed.get('token') || null;
}

export function BlogDraftPreview({ locale, tenantSlug }: BlogDraftPreviewProps) {
  const t = useTranslations('Blog');
  const [tokenInput, setTokenInput] = useState('');
  const [post, setPost] = useState<BlogPreviewPostDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const [notFound, setNotFound] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);

  const openPreview = useCallback(
    async (token: string) => {
      const trimmed = token.trim();
      if (!trimmed || loading) return;
      setLoading(true);
      setNotFound(false);
      setFailure(null);
      try {
        const resolved = await fetchPostByPreviewToken(
          storefrontGraphql,
          tenantSlug,
          trimmed,
          locale
        );
        if (!resolved) {
          setPost(null);
          setNotFound(true);
          return;
        }
        setPost(resolved);
      } catch {
        setPost(null);
        setFailure(
          t('preview.loadError')
        );
      } finally {
        setLoading(false);
      }
    },
    [locale, loading, tenantSlug, t]
  );

  useEffect(() => {
    const token = tokenFromHash(window.location.hash);
    if (token) {
      setTokenInput(token);
      void openPreview(token);
    }
  }, [openPreview]);

  useEffect(() => {
    const onHashChange = () => {
      const token = tokenFromHash(window.location.hash);
      if (token) {
        setTokenInput(token);
        void openPreview(token);
      }
    };
    window.addEventListener('hashchange', onHashChange);
    return () => window.removeEventListener('hashchange', onHashChange);
  }, [openPreview]);

  const publishedDate = post?.publishedAt
    ? new Date(post.publishedAt).toLocaleDateString(locale, {
        year: 'numeric',
        month: 'long',
        day: 'numeric',
      })
    : null;
  const readingMinutes = post ? calculateReadingTime(post.contentPlainText) : 0;
  const readingTimeLabel = post ? formatReadingTime(readingMinutes, t) : null;

  if (post) {
    return (
      <article className='mx-auto max-w-6xl px-4 sm:px-6 py-10 space-y-8'>
        <div className='rounded-lg border border-amber-500/40 bg-amber-500/10 px-4 py-3 text-sm text-amber-700 dark:text-amber-400'>
          <span className='inline-flex items-center gap-2 font-medium'>
            <Eye className='h-4 w-4' />
            {t('preview.title')}
          </span>
          <p className='mt-1 text-muted-foreground'>
            {t('preview.banner')}
          </p>
        </div>

        <header className='space-y-4 border-b border-border pb-6'>
          <div className='flex flex-wrap items-center gap-4 text-xs text-muted-foreground'>
            {post.categoryName && (
              <span className='rounded-full bg-secondary px-2.5 py-0.5 font-medium text-secondary-foreground text-xs'>
                {post.categoryName}
              </span>
            )}
            {post.authorProfile && (
              <AuthorMiniBadge author={post.authorProfile} locale={locale} />
            )}
            {publishedDate && (
              <span className='inline-flex items-center gap-1.5'>
                <Calendar className='h-3.5 w-3.5' />
                <span>{publishedDate}</span>
              </span>
            )}
            {readingTimeLabel && (
              <span className='inline-flex items-center gap-1.5'>
                <Clock className='h-3.5 w-3.5' />
                <span>{readingTimeLabel}</span>
              </span>
            )}
            {post.tags.length > 0 && (
              <div className='flex flex-wrap items-center gap-1.5'>
                <Tag className='h-3 w-3 text-muted-foreground' />
                {post.tags.map((tag) => (
                  <span
                    key={tag}
                    className='rounded-full bg-primary/10 px-2.5 py-0.5 font-medium text-primary text-[11px]'
                  >
                    #{tag}
                  </span>
                ))}
              </div>
            )}
          </div>

          <h1 className='text-3xl font-extrabold tracking-tight text-foreground sm:text-4xl lg:text-5xl leading-tight'>
            {post.title}
          </h1>

          {post.excerpt && (
            <p className='text-lg text-muted-foreground leading-relaxed'>{post.excerpt}</p>
          )}
        </header>

        <div className='space-y-8'>
          {post.featuredImageUrl && (
            <div className='relative aspect-video w-full overflow-hidden rounded-2xl border border-border shadow-sm'>
              <Image
                src={post.featuredImageUrl}
                unoptimized
                alt={post.title}
                fill
                className='object-cover'
                priority
              />
            </div>
          )}
          <div className='prose prose-neutral dark:prose-invert max-w-none pt-2'>
            <RichTextHtml
              view={post.content}
              contentLocale={post.effectiveLocale}
              className='text-base leading-8 text-foreground/90 space-y-4'
            />
          </div>
        </div>
      </article>
    );
  }

  return (
    <div className='mx-auto max-w-xl px-4 sm:px-6 py-16 space-y-6'>
      <h1 className='text-2xl font-bold tracking-tight text-foreground'>
        {t('preview.title')}
      </h1>
      <p className='text-sm text-muted-foreground'>
        {t('preview.hint')}
      </p>
      <form
        className='flex items-center gap-2'
        onSubmit={(event) => {
          event.preventDefault();
          void openPreview(tokenInput);
        }}
      >
        <input
          type='text'
          value={tokenInput}
          onChange={(event) => setTokenInput(event.target.value)}
          placeholder='token'
          autoComplete='off'
          spellCheck={false}
          className='flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2'
        />
        <Button
          type='submit'
          disabled={loading || !tokenInput.trim()}
          className='inline-flex h-10 items-center justify-center whitespace-nowrap rounded-md bg-primary px-4 text-sm font-medium text-primary-foreground transition-colors disabled:pointer-events-none disabled:opacity-50'
        >
          {loading
            ? t('preview.loading')
            : t('preview.open')}
        </Button>
      </form>
      {notFound && (
        <p className='text-sm text-destructive'>
          {t('preview.invalid')}
        </p>
      )}
      {failure && <p className='text-sm text-destructive'>{failure}</p>}
    </div>
  );
}
