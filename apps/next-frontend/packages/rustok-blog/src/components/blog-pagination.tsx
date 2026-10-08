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
import { ChevronLeft, ChevronRight } from 'lucide-react';

/**
 * Cursor pagination for the public blog list. The backend is keyset-based, so
 * there are no page numbers and no total. "First page" resets the cursor; the
 * previous cursor is not kept, so there is no step-back control.
 */
export function BlogPagination({
  nextCursor,
  currentCursor,
  baseUrl,
  selectedTag,
  selectedCategory,
  searchQuery,
  locale = 'en',
}: {
  nextCursor: string | null;
  currentCursor: string | null;
  baseUrl: string;
  selectedTag?: string;
  selectedCategory?: string;
  searchQuery?: string;
  locale?: string;
}): React.JSX.Element | null {
  if (!nextCursor && !currentCursor) {
    return null;
  }

  const isRu = locale === 'ru';

  function makeUrl(after: string | null): string {
    const params = new URLSearchParams();
    if (selectedTag) {
      params.set('tag', selectedTag);
    }
    if (selectedCategory) {
      params.set('category', selectedCategory);
    }
    if (searchQuery) {
      params.set('q', searchQuery);
    }
    if (after) {
      params.set('after', after);
    }
    const qs = params.toString();
    return qs ? `${baseUrl}?${qs}` : baseUrl;
  }

  const linkClass =
    'inline-flex items-center gap-1 rounded-xl border border-border bg-card px-3 py-2 text-xs font-medium text-foreground hover:bg-muted transition';
  const disabledClass =
    'inline-flex items-center gap-1 rounded-xl border border-border/40 bg-card px-3 py-2 text-xs font-medium text-muted-foreground/40 cursor-not-allowed';

  return (
    <nav
      aria-label={isRu ? 'Навигация по записям' : 'Posts pagination'}
      className="flex flex-wrap items-center justify-center gap-2 pt-8"
    >
      {currentCursor ? (
        <Link href={makeUrl(null)} className={linkClass}>
          <ChevronLeft className="h-3.5 w-3.5" />
          <span>{isRu ? 'В начало' : 'First page'}</span>
        </Link>
      ) : (
        <span className={disabledClass}>
          <ChevronLeft className="h-3.5 w-3.5" />
          <span>{isRu ? 'В начало' : 'First page'}</span>
        </span>
      )}

      {nextCursor ? (
        <Link href={makeUrl(nextCursor)} className={linkClass}>
          <span>{isRu ? 'Вперед' : 'Next'}</span>
          <ChevronRight className="h-3.5 w-3.5" />
        </Link>
      ) : (
        <span className={disabledClass}>
          <span>{isRu ? 'Вперед' : 'Next'}</span>
          <ChevronRight className="h-3.5 w-3.5" />
        </span>
      )}
    </nav>
  );
}
