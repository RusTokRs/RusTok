/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import React from 'react';
import Image from 'next/image';
import Link from 'next/link';
import { Calendar, User } from 'lucide-react';
import type { BlogPostSummary } from '../api/posts';

export function PostCard({
  post,
  href,
  locale = 'en',
}: {
  post: BlogPostSummary;
  href: string | null;
  locale?: string;
}): React.JSX.Element {
  const isRu = locale === 'ru';
  const date = post.publishedAt
    ? new Date(post.publishedAt).toLocaleDateString(locale, {
        year: 'numeric',
        month: 'short',
        day: 'numeric',
      })
    : null;

  const authorInitials = post.authorProfile
    ? post.authorProfile.displayName
        .trim()
        .split(/\s+/)
        .slice(0, 2)
        .map((p) => p[0])
        .join('')
        .toUpperCase()
    : null;

  return (
    <article className="group flex flex-col rounded-2xl border border-border bg-card overflow-hidden shadow-sm transition-all hover:shadow-md hover:border-primary/30">
      {post.featuredImageUrl && (
        <div className="relative aspect-video w-full overflow-hidden bg-muted">
          {href ? (
            <Link href={href} className="block h-full w-full">
              <Image
                src={post.featuredImageUrl}
                alt={post.title}
                width={800}
                height={450}
                className="h-full w-full object-cover transition-transform duration-300 group-hover:scale-105"
              />
            </Link>
          ) : (
            <Image
              src={post.featuredImageUrl}
              alt={post.title}
              width={800}
              height={450}
              className="h-full w-full object-cover"
            />
          )}
        </div>
      )}

      <div className="flex flex-1 flex-col p-5 space-y-3">
        {/* Meta row: Category, Author & Date */}
        <div className="flex flex-wrap items-center gap-2.5 text-xs text-muted-foreground">
          {post.categoryName && (
            <span className="rounded-full bg-secondary px-2.5 py-0.5 font-medium text-secondary-foreground text-[11px]">
              {post.categoryName}
            </span>
          )}

          {post.authorProfile ? (
            <div className="inline-flex items-center gap-1.5 font-medium text-foreground">
              <div className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-primary/10 text-primary text-[10px] font-bold">
                {authorInitials}
              </div>
              <span className="truncate max-w-[120px]">{post.authorProfile.displayName}</span>
            </div>
          ) : null}

          {date && (
            <span className="inline-flex items-center gap-1">
              <Calendar className="h-3 w-3" />
              <span>{date}</span>
            </span>
          )}
        </div>

        {/* Title */}
        <h3 className="text-lg font-bold text-card-foreground leading-snug group-hover:text-primary transition-colors">
          {href ? (
            <Link href={href}>
              {post.title}
            </Link>
          ) : (
            post.title
          )}
        </h3>

        {/* Excerpt */}
        {post.excerpt && (
          <p className="line-clamp-2 text-sm text-muted-foreground leading-relaxed">
            {post.excerpt}
          </p>
        )}

        {/* Tags and Action */}
        <div className="mt-auto pt-3 flex flex-wrap items-center justify-between gap-2 border-t border-border/50">
          <div className="flex flex-wrap gap-1">
            {post.tags.slice(0, 3).map((tag) => (
              <span
                key={tag}
                className="rounded-full bg-primary/10 px-2 py-0.5 text-[11px] font-medium text-primary"
              >
                #{tag}
              </span>
            ))}
            {post.tags.length > 3 && (
              <span className="text-[11px] text-muted-foreground self-center">
                +{post.tags.length - 3}
              </span>
            )}
          </div>

          {href && (
            <Link
              href={href}
              className="text-xs font-semibold text-primary hover:underline ml-auto"
            >
              {isRu ? 'Читать →' : 'Read →'}
            </Link>
          )}
        </div>
      </div>
    </article>
  );
}
