/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import type { BlogPostDetail } from '../api/posts';

/**
 * Builds BlogPosting JSON-LD. `url` must be an absolute URL (schema.org
 * `mainEntityOfPage` requires it to identify the page unambiguously).
 */
export function buildArticleJsonLd({
  post,
  url,
}: {
  post: BlogPostDetail;
  url: string;
}): Record<string, unknown> {
  const schema: Record<string, unknown> = {
    '@context': 'https://schema.org',
    '@type': 'BlogPosting',
    mainEntityOfPage: {
      '@type': 'WebPage',
      '@id': url,
    },
    headline: post.seoTitle || post.title,
    description: post.seoDescription || post.excerpt || post.title,
    inLanguage: post.effectiveLocale,
  };

  if (post.featuredImageUrl) {
    schema.image = [post.featuredImageUrl];
  }

  if (post.publishedAt) {
    schema.datePublished = post.publishedAt;
  }

  if (post.updatedAt) {
    schema.dateModified = post.updatedAt;
  }

  if (post.authorProfile) {
    schema.author = {
      '@type': 'Person',
      name: post.authorProfile.displayName,
      identifier: post.authorProfile.handle,
    };
  }

  if (post.tags && post.tags.length > 0) {
    schema.keywords = post.tags.join(', ');
  }

  schema.publisher = {
    '@type': 'Organization',
    name: 'RusToK',
  };

  return schema;
}

export function serializeJsonLd(payload: unknown): string {
  if (payload === null || payload === undefined || typeof payload !== 'object') {
    return '{}';
  }
  return JSON.stringify(payload)
    .replace(/</g, '\\u003c')
    .replace(/>/g, '\\u003e')
    .replace(/&/g, '\\u0026')
    .replace(/\u2028/g, '\\u2028')
    .replace(/\u2029/g, '\\u2029');
}
