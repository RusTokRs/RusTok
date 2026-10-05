'use client';

/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import React, { useEffect, useState } from 'react';
import { AlignLeft } from 'lucide-react';

export interface TocItem {
  id: string;
  text: string;
  level: number;
}

export function BlogTableOfContents({
  contentSelector = '#article-body',
  locale = 'en',
}: {
  contentSelector?: string;
  locale?: string;
}): React.JSX.Element | null {
  const [headings, setHeadings] = useState<TocItem[]>([]);
  const [activeId, setActiveId] = useState<string>('');

  useEffect(() => {
    const container = document.querySelector(contentSelector);
    if (!container) return;

    const elements = container.querySelectorAll<HTMLHeadingElement>('h2, h3');
    const items: TocItem[] = [];

    elements.forEach((el, index) => {
      const text = el.textContent?.trim() || '';
      if (!text) return;

      let id = el.id;
      if (!id) {
        id = `section-${index + 1}`;
        el.id = id;
      }

      const level = el.tagName.toLowerCase() === 'h3' ? 3 : 2;
      items.push({ id, text, level });
    });

    setHeadings(items);

    if (items.length === 0) return;

    // IntersectionObserver to track active heading
    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            setActiveId(entry.target.id);
          }
        });
      },
      {
        rootMargin: '0px 0px -70% 0px',
        threshold: 0.1,
      }
    );

    elements.forEach((el) => observer.observe(el));

    return () => {
      observer.disconnect();
    };
  }, [contentSelector]);

  if (headings.length < 2) {
    return null;
  }

  const title = locale === 'ru' ? 'Содержание' : 'Table of Contents';

  const handleClick = (e: React.MouseEvent<HTMLAnchorElement>, id: string) => {
    e.preventDefault();
    const target = document.getElementById(id);
    if (target) {
      target.scrollIntoView({ behavior: 'smooth', block: 'start' });
      setActiveId(id);
      window.history.pushState(null, '', `#${id}`);
    }
  };

  return (
    <nav
      aria-label={title}
      className="sticky top-24 rounded-2xl border border-border bg-card p-5 shadow-sm space-y-3"
    >
      <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground border-b border-border pb-3">
        <AlignLeft className="h-4 w-4 text-primary" />
        <span>{title}</span>
      </div>
      <ul className="space-y-1.5 text-sm max-h-[calc(100vh-12rem)] overflow-y-auto pr-1">
        {headings.map((item) => {
          const isActive = activeId === item.id;
          return (
            <li key={item.id} className={item.level === 3 ? 'pl-3' : ''}>
              <a
                href={`#${item.id}`}
                onClick={(e) => handleClick(e, item.id)}
                className={`block py-1 text-xs transition-colors rounded-md px-2 ${
                  isActive
                    ? 'bg-primary/10 font-semibold text-primary'
                    : 'text-muted-foreground hover:bg-muted hover:text-foreground'
                }`}
              >
                {item.text}
              </a>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
