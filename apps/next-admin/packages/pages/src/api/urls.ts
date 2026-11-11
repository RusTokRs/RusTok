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
export function builderUrlForPage(
  builderBaseUrl: string,
  pageId: string
): string {
  const base = builderBaseUrl.replace(/\/+$/, '');
  return `${base}/modules/pages?pageId=${encodeURIComponent(pageId)}`;
}

/**
 * Storefront URL of a published page.
 *
 * The storefront renders the published landing only, so a page that is not
 * published has no storefront URL: `null`. The working copy is previewed in the
 * builder, never on the storefront.
 */
export function storefrontUrlForPage(
  storefrontBaseUrl: string,
  slug: string,
  locale = 'en',
  status: PageStatus = 'published'
): string | null {
  if (status !== 'published') {
    return null;
  }
  const base = storefrontBaseUrl.replace(/\/+$/, '');
  const cleanSlug = slug.replace(/^\/+/, '');
  return `${base}/${encodeURIComponent(locale)}/pages/${encodeURIComponent(cleanSlug)}`;
}
