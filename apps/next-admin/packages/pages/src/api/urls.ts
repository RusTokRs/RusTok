/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

export type PageStatus = 'draft' | 'published' | 'archived';

/**
 * Deep link into the Rust (FFA/Leptos) page-builder editor for one page.
 */
export function builderUrlForPage(builderBaseUrl: string, pageId: string): string {
  const base = builderBaseUrl.replace(/\/+$/, '');
  return `${base}/modules/pages?pageId=${encodeURIComponent(pageId)}`;
}

/**
 * Generates storefront URL or live draft preview link for a page.
 * If status is draft and previewSecret is provided, links to /api/draft.
 */
export function storefrontUrlForPage(
  storefrontBaseUrl: string,
  slug: string,
  locale = 'en',
  status: PageStatus = 'published',
  previewSecret?: string
): string {
  const base = storefrontBaseUrl.replace(/\/+$/, '');
  if (status === 'draft' && previewSecret) {
    return `${base}/api/draft?slug=${encodeURIComponent(slug)}&locale=${encodeURIComponent(locale)}&secret=${encodeURIComponent(previewSecret)}`;
  }
  return `${base}/${encodeURIComponent(locale)}/pages/${encodeURIComponent(slug)}`;
}
