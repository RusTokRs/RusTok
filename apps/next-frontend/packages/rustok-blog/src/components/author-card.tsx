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
import Link from 'next/link';
import { User } from 'lucide-react';
import type { BlogPostAuthorProfile } from '../api/posts';

function getInitials(name: string, handle: string): string {
  const parts = name.trim().split(/\s+/).filter(Boolean);
  const initials = parts
    .slice(0, 2)
    .map((p) => p[0])
    .join('')
    .toUpperCase();
  if (initials) return initials;
  return handle ? handle[0].toUpperCase() : '?';
}

export function AuthorMiniBadge({
  author,
  locale = 'en',
}: {
  author: BlogPostAuthorProfile;
  locale?: string;
}): React.JSX.Element {
  const initials = getInitials(author.displayName, author.handle);

  return (
    <Link
      href={`/${locale}/profiles/${encodeURIComponent(author.handle)}`}
      className="inline-flex items-center gap-2 group transition-opacity hover:opacity-85"
    >
      <div className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-primary/10 text-primary text-[11px] font-bold ring-1 ring-border group-hover:ring-primary/40">
        {initials}
      </div>
      <span className="text-xs font-medium text-foreground group-hover:text-primary transition-colors">
        {author.displayName}
      </span>
    </Link>
  );
}

export function AuthorBioCard({
  author,
  locale = 'en',
}: {
  author: BlogPostAuthorProfile;
  locale?: string;
}): React.JSX.Element {
  const initials = getInitials(author.displayName, author.handle);
  const isRu = locale === 'ru';

  return (
    <section className="rounded-2xl border border-border bg-card p-6 shadow-sm">
      <div className="flex items-start gap-4">
        <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl bg-primary text-xl font-bold text-primary-foreground shadow-sm">
          {initials}
        </div>
        <div className="space-y-1.5 flex-1 min-w-0">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div>
              <h3 className="text-base font-bold text-foreground">
                {author.displayName}
              </h3>
              <p className="text-xs text-muted-foreground">
                @{author.handle}
              </p>
            </div>
            <Link
              href={`/${locale}/profiles/${encodeURIComponent(author.handle)}`}
              className="rounded-full border border-border bg-background px-3 py-1 text-xs font-medium text-foreground hover:bg-accent transition"
            >
              {isRu ? 'Профиль автора' : 'View Profile'}
            </Link>
          </div>
          {author.tags && author.tags.length > 0 && (
            <div className="flex flex-wrap gap-1 pt-1">
              {author.tags.map((tag) => (
                <span
                  key={tag}
                  className="rounded-md bg-muted px-2 py-0.5 text-[10px] font-medium text-muted-foreground"
                >
                  {tag}
                </span>
              ))}
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
