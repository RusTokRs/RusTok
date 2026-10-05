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

export function CommentsPagination({
  currentPage,
  totalItems,
  pageSize = 20,
  baseUrl,
  locale = 'en',
}: {
  currentPage: number;
  totalItems: number;
  pageSize?: number;
  baseUrl: string;
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
    return page === 1 ? `${baseUrl}#comments` : `${baseUrl}?commentsPage=${page}#comments`;
  }

  return (
    <nav
      aria-label={isRu ? 'Пагинация комментариев' : 'Comments pagination'}
      className="flex items-center justify-between border-t border-border pt-4 text-xs"
    >
      <div>
        {currentPage > 1 ? (
          <Link
            href={makeUrl(prevPage)}
            className="inline-flex items-center gap-1 rounded-lg border border-border px-3 py-1.5 font-medium text-foreground hover:bg-muted transition"
          >
            <ChevronLeft className="h-3.5 w-3.5" />
            <span>{isRu ? 'Назад' : 'Previous'}</span>
          </Link>
        ) : (
          <span className="inline-flex items-center gap-1 rounded-lg border border-border/40 px-3 py-1.5 font-medium text-muted-foreground/50 cursor-not-allowed">
            <ChevronLeft className="h-3.5 w-3.5" />
            <span>{isRu ? 'Назад' : 'Previous'}</span>
          </span>
        )}
      </div>

      <div className="text-muted-foreground font-medium">
        {isRu
          ? `Страница ${currentPage} из ${totalPages}`
          : `Page ${currentPage} of ${totalPages}`}
      </div>

      <div>
        {currentPage < totalPages ? (
          <Link
            href={makeUrl(nextPage)}
            className="inline-flex items-center gap-1 rounded-lg border border-border px-3 py-1.5 font-medium text-foreground hover:bg-muted transition"
          >
            <span>{isRu ? 'Вперед' : 'Next'}</span>
            <ChevronRight className="h-3.5 w-3.5" />
          </Link>
        ) : (
          <span className="inline-flex items-center gap-1 rounded-lg border border-border/40 px-3 py-1.5 font-medium text-muted-foreground/50 cursor-not-allowed">
            <span>{isRu ? 'Вперед' : 'Next'}</span>
            <ChevronRight className="h-3.5 w-3.5" />
          </span>
        )}
      </div>
    </nav>
  );
}
