/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import { storefrontGraphql } from "@/shared/lib/graphql";
import {
  type PageDetail,
  type PageTranslation,
  type PublishedLanding,
  STOREFRONT_PAGE_QUERY,
  readStorefrontPage,
} from "./page-query";

export type { PageDetail, PageTranslation, PublishedLanding };

/**
 * Fetches a published page with its published landing in one request, so the
 * page and the markup that is rendered for it come from the same read.
 * Returns `null` when no published page matches the slug.
 */
export async function fetchPublishedPage(
  slug: string,
  locale?: string,
  tenantSlug?: string | null,
  tenantId?: string | null,
): Promise<PageDetail | null> {
  const response = await storefrontGraphql<
    { selectedPage: PageDetail | null },
    { pageSlug: string; locale?: string; tenantId?: string }
  >({
    query: STOREFRONT_PAGE_QUERY,
    variables: {
      pageSlug: slug,
      locale,
      tenantId: tenantId ?? undefined,
    },
    tenant: tenantSlug ?? undefined,
  });

  return readStorefrontPage(response);
}
