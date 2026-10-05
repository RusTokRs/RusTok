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

export function BlogPagination({
  currentPage,
  totalItems,
  pageSize = 9,
  baseUrl,
  selectedTag,
  locale = 'en',
}: {
  currentPage: number;
  totalItems: number;
  pageSize?: number;
  baseUrl: string;
  selectedTag?: string;
  locale?: string;
}): React.JSX.Element | null {
  const totalPages = Math.max(1, Math.ceil(totalItems / pageSize));
  if (totalPages <= 1) {
    return null;
  }

  const isRu = locale === 'ru';
  const prevPage = currentPage - 1;
  const nextPage = currentPage + 1;

  function makeUrl(page: number): string {
    const params = new URLSearchParams();
    if (selectedTag) {
      params.set('tag', selectedTag);
    }
    if (page > 1) {
      params.set('page', page.toString());
    }
    const qs = params.toString();
    return qs ? `${baseUrl}?${qs}` : baseUrl;
  }

  // Generate page numbers to display
  const pages: number[] = [];
  const startPage = Math.max(1, currentPage - 2);
  const endPage = Math.min(totalPages, currentPage + 2);
  for (let i = startPage; i <= endPage; i++) {
    pages.push(i);
  }

  return (
    <nav
      aria-label={isRu ? 'Навигация по страницам' : 'Posts pagination'}
      className="flex flex-wrap items-center justify-center gap-2 pt-8"
    >
      {currentPage > 1 ? (
        <Link
          href={makeUrl(prevPage)}
          className="inline-flex items-center gap-1 rounded-xl border border-border bg-card px-3 py-2 text-xs font-medium text-foreground hover:bg-muted transition"
        >
          <ChevronLeft className="h-3.5 w-3.5" />
          <span>{isRu ? 'Назад' : 'Previous'}</span>
        </Link>
      ) : (
        <span className="inline-flex items-center gap-1 rounded-xl border border-border/40 bg-card px-3 py-2 text-xs font-medium text-muted-foreground/40 cursor-not-allowed">
          <ChevronLeft className="h-3.5 w-3.5" />
          <span>{isRu ? 'Назад' : 'Previous'}</span>
        </span>
      )}

      {pages.map((p) => {
        const isActive = p === currentPage;
        return (
          <Link
            key={p}
            href={makeUrl(p)}
            className={`inline-flex h-9 w-9 items-center justify-center rounded-xl text-xs font-semibold transition ${
              isActive
                ? 'bg-primary text-primary-foreground shadow-sm'
                : 'border border-border bg-card text-foreground hover:bg-muted'
            }`}
          >
            {p}
          </Link>
        );
      })}

      {currentPage < totalPages ? (
        <Link
          href={makeUrl(nextPage)}
          className="inline-flex items-center gap-1 rounded-xl border border-border bg-card px-3 py-2 text-xs font-medium text-foreground hover:bg-muted transition"
        >
          <span>{isRu ? 'Вперед' : 'Next'}</span>
          <ChevronRight className="h-3.5 w-3.5" />
        </Link>
      ) : (
        <span className="inline-flex items-center gap-1 rounded-xl border border-border/40 bg-card px-3 py-2 text-xs font-medium text-muted-foreground/40 cursor-not-allowed">
          <span>{isRu ? 'Вперед' : 'Next'}</span>
          <ChevronRight className="h-3.5 w-3.5" />
        </span>
      )}
    </nav>
  );
}
