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

export interface TableOfContentsProps {
  /**
   * CSS selector for the content container containing headings.
   * Defaults to targeting rich text containers and standard content ids
   * across blog, forum, product description, and documentation.
   */
  contentSelector?: string;
  /**
   * Current locale ('ru', 'en', etc.) used for default labels.
   */
  locale?: string;
  /**
   * Minimum number of headings required to render the table of contents.
   * If fewer headings are detected, the component returns null.
   * Defaults to 2.
   */
  minHeadings?: number;
  /**
   * Heading tag levels to include in the table of contents.
   * Defaults to ['h2', 'h3'].
   */
  levels?: ('h2' | 'h3' | 'h4')[];
  /**
   * Whether to apply sticky positioning.
   * Defaults to true.
   */
  sticky?: boolean;
  /**
   * Custom title override for the table of contents.
   * If omitted, localized string is used based on `locale`.
   */
  title?: string;
  /**
   * Optional custom CSS class name for the root nav element.
   */
  className?: string;
}

function generateSlug(text: string, index: number): string {
  const clean = text
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, '-')
    .replace(/^-+|-+$/g, '');
  return clean ? clean : `section-${index + 1}`;
}

/**
 * Universal, accessible Table of Contents component.
 *
 * Canonical shared component for long-form content across RusToK storefront:
 * - Blog articles (`/blog/[slug]`)
 * - Forum topics and threads (`/forum/[slug]`)
 * - Long product descriptions (`/products/[slug]`)
 * - Documentation and knowledge base pages
 *
 * Automatically inspects rich-text and article containers, attaches deterministic
 * anchors if missing, tracks the active section via IntersectionObserver, and provides
 * smooth scrolling with hash persistence.
 */
export function TableOfContents({
  contentSelector = '[data-richtext], #article-body, #product-description, #forum-post, main article',
  locale = 'en',
  minHeadings = 2,
  levels = ['h2', 'h3'],
  sticky = true,
  title,
  className = '',
}: TableOfContentsProps): React.JSX.Element | null {
  const [headings, setHeadings] = useState<TocItem[]>([]);
  const [activeId, setActiveId] = useState<string>('');

  useEffect(() => {
    // Try to find the container matching contentSelector
    const selectors = contentSelector.split(',').map((s) => s.trim());
    let container: Element | null = null;
    for (const sel of selectors) {
      const match = document.querySelector(sel);
      if (match) {
        container = match;
        break;
      }
    }

    if (!container) return;

    const queryTags = levels.join(', ');
    const elements = container.querySelectorAll<HTMLHeadingElement>(queryTags);
    const items: TocItem[] = [];

    elements.forEach((el, index) => {
      const text = el.textContent?.trim() || '';
      if (!text) return;

      let id = el.id;
      if (!id) {
        id = generateSlug(text, index);
        // Ensure uniqueness if same heading text repeats
        if (items.some((item) => item.id === id)) {
          id = `${id}-${index + 1}`;
        }
        el.id = id;
      }

      const tagName = el.tagName.toLowerCase();
      const level = tagName === 'h4' ? 4 : tagName === 'h3' ? 3 : 2;
      items.push({ id, text, level });
    });

    setHeadings(items);

    if (items.length < minHeadings) return;

    // IntersectionObserver to dynamically highlight current heading
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
  }, [contentSelector, minHeadings, levels]);

  if (headings.length < minHeadings) {
    return null;
  }

  const effectiveTitle =
    title ?? (locale === 'ru' ? 'Содержание' : 'Table of Contents');

  const handleClick = (e: React.MouseEvent<HTMLAnchorElement>, id: string) => {
    e.preventDefault();
    const target = document.getElementById(id);
    if (target) {
      target.scrollIntoView({ behavior: 'smooth', block: 'start' });
      setActiveId(id);
      window.history.pushState(null, '', `#${id}`);
    }
  };

  const stickyClasses = sticky ? 'sticky top-24' : '';

  return (
    <nav
      aria-label={effectiveTitle}
      className={`${stickyClasses} rounded-2xl border border-border bg-card p-5 shadow-sm space-y-3 ${className}`.trim()}
    >
      <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground border-b border-border pb-3">
        <AlignLeft className="h-4 w-4 text-primary" />
        <span>{effectiveTitle}</span>
      </div>
      <ul
        role="list"
        className="space-y-1.5 text-sm max-h-[calc(100vh-12rem)] overflow-y-auto pr-1"
      >
        {headings.map((item) => {
          const isActive = activeId === item.id;
          const paddingClass =
            item.level === 4 ? 'pl-6' : item.level === 3 ? 'pl-3' : '';

          return (
            <li key={item.id} className={paddingClass}>
              <a
                href={`#${item.id}`}
                onClick={(e) => handleClick(e, item.id)}
                aria-current={isActive ? 'location' : undefined}
                className={`block py-1 text-xs transition-colors rounded-md px-2 ${
                  isActive
                    ? 'bg-primary/10 font-semibold text-primary border-l-2 border-primary'
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

export default TableOfContents;
