/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import { defaultGraphqlBaseUrl, storefrontGraphql } from "@/shared/lib/graphql";
import { type ArtifactContent, parseArtifactHtml } from "./artifact";

export type { ArtifactContent };
export { parseArtifactHtml };

export interface PageTranslation {
  locale: string;
  title: string | null;
  slug: string | null;
  metaTitle: string | null;
  metaDescription: string | null;
}

export interface PageBody {
  locale: string;
  content: string;
  format: string;
}

export interface PageDetail {
  id: string;
  version: number;
  status: string;
  effectiveLocale: string | null;
  translation: PageTranslation | null;
  body: PageBody | null;
}

const STOREFRONT_PAGE_QUERY = `
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
      body {
        locale
        content
        format
      }
    }
  }
`;

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

  if (response.errors?.length) {
    throw new Error(response.errors[0]?.message ?? "Page query failed");
  }

  return response.data?.selectedPage ?? null;
}


export async function fetchPageArtifact(
  artifactUrlOrContent: string,
  format: string,
  baseUrl = defaultGraphqlBaseUrl(),
): Promise<ArtifactContent> {
  if (format === "fly_artifact_url" || artifactUrlOrContent.startsWith("/api/pages/")) {
    const fullUrl = artifactUrlOrContent.startsWith("http")
      ? artifactUrlOrContent
      : `${baseUrl.replace(/\/$/, "")}/${artifactUrlOrContent.replace(/^\//, "")}`;

    const res = await fetch(fullUrl, {
      next: { revalidate: 60 },
      headers: {
        Accept: "text/html",
      },
    });

    if (!res.ok) {
      throw new Error(`Failed to fetch page artifact: HTTP ${res.status}`);
    }

    const documentHtml = await res.text();
    return parseArtifactHtml(documentHtml);
  }

  if (artifactUrlOrContent.trim().startsWith("<!doctype") || artifactUrlOrContent.includes("<html")) {
    return parseArtifactHtml(artifactUrlOrContent);
  }

  return {
    html: `<div class="fly-raw-content">${artifactUrlOrContent}</div>`,
    css: "",
  };
}
