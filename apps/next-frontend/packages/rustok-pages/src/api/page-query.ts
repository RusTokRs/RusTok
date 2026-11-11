/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

/**
 * The published landing of one page locale: the sanitized body markup and the
 * stylesheet stored by the reviewed publish (`GqlPublishedLanding` on the
 * server). It is the only page content the storefront renders.
 */
export interface PublishedLanding {
  /** Locale the landing was published for; a fallback locale can differ from the requested one. */
  locale: string;
  /** Sanitized body markup. */
  html: string;
  /** Stylesheet of the landing, not yet scoped to a host element. */
  css: string;
  /** Identity of the artifact; changes whenever the markup or the stylesheet changes. */
  artifactHash: string;
}

export interface PageTranslation {
  locale: string;
  title: string | null;
  slug: string | null;
  metaTitle: string | null;
  metaDescription: string | null;
}

export interface PageDetail {
  id: string;
  version: number;
  status: string;
  effectiveLocale: string | null;
  translation: PageTranslation | null;
  /**
   * `null` when the page has no published landing for the effective locale on
   * the request channel.
   */
  landing: PublishedLanding | null;
}

/**
 * One read of a published page: its identity, SEO fields and published landing.
 *
 * The page `body` is deliberately absent. It is the Page Builder authoring
 * document, which the server returns only to callers that may save or publish
 * it and which is not sanitized for rendering.
 */
export const STOREFRONT_PAGE_QUERY = `
  query StorefrontPage($pageSlug: String!, $locale: String, $tenantId: UUID) {
    selectedPage: pageBySlug(slug: $pageSlug, locale: $locale, tenantId: $tenantId) {
      id
      version
      status
      effectiveLocale
      translation {
        locale
        title
        slug
        metaTitle
        metaDescription
      }
      landing {
        locale
        html
        css
        artifactHash
      }
    }
  }
`;

export interface StorefrontPageResponse {
  data?: { selectedPage: PageDetail | null };
  errors?: { message: string }[];
}

/**
 * Reads the response of {@link STOREFRONT_PAGE_QUERY}: the page, or `null` when
 * no published page matches the slug. Any GraphQL error fails the read, also
 * when partial data came with it: a page that lost its landing to an error must
 * not look like a page without a landing.
 */
export function readStorefrontPage(response: StorefrontPageResponse): PageDetail | null {
  if (response.errors?.length) {
    throw new Error(response.errors[0]?.message || "Page query failed");
  }
  return response.data?.selectedPage ?? null;
}
