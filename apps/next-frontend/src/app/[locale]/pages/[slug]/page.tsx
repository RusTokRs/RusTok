/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { cache } from "react";
import {
  getStorefrontTenantId,
  getStorefrontTenantSlug,
} from "@/shared/api/modules";
import { buildSeoMetadata } from "@/shared/seo/metadata";
import { resolveSeoPageContextForRoute } from "@/shared/seo/runtime";
import { fetchPublishedPage, PageBuilderContent } from "@rustok/pages-frontend";

// `generateMetadata` and the page render the same page: one read per request.
const loadPublishedPage = cache(fetchPublishedPage);

interface StorefrontPageProps {
  params: Promise<{ locale: string; slug: string }>;
}

export async function generateMetadata({
  params,
}: StorefrontPageProps): Promise<Metadata> {
  const { locale, slug } = await params;
  const tenantSlug = getStorefrontTenantSlug();
  const tenantId = getStorefrontTenantId();

  let page = null;
  try {
    page = await loadPublishedPage(slug, locale, tenantSlug, tenantId);
  } catch {
    // Fail gracefully for metadata generation
  }
  if (page && page.status !== "published") {
    page = null;
  }

  const routePath = `/pages/${encodeURIComponent(slug)}`;
  const seoResolution = await resolveSeoPageContextForRoute({
    locale,
    route: routePath,
  });

  const pageTitle = page?.translation?.title || page?.translation?.metaTitle || slug;
  const pageDescription =
    page?.translation?.metaDescription || `Published page ${slug}`;

  return buildSeoMetadata({
    locale,
    title: pageTitle,
    description: pageDescription,
    path: routePath,
    context: seoResolution.context,
  });
}

export default async function StorefrontCustomPage({
  params,
}: StorefrontPageProps) {
  const { locale, slug } = await params;
  const tenantSlug = getStorefrontTenantSlug();
  const tenantId = getStorefrontTenantId();

  let page = null;
  try {
    page = await loadPublishedPage(slug, locale, tenantSlug, tenantId);
  } catch (error) {
    console.error(`[StorefrontPage] Failed to fetch page "${slug}":`, error);
    notFound();
  }

  // Only the published landing is rendered: it is the output of the reviewed
  // publish. A page without one has nothing to show; there is no other content
  // source to fall back to.
  const landing = page?.status === "published" ? page.landing : null;
  if (!landing) {
    notFound();
  }

  return (
    <main className="rustok-page-wrapper min-h-screen">
      <PageBuilderContent html={landing.html} css={landing.css} />
    </main>
  );
}
