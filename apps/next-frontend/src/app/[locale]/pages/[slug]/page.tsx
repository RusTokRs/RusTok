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
import { draftMode } from "next/headers";
import { notFound } from "next/navigation";
import {
  getStorefrontTenantId,
  getStorefrontTenantSlug,
} from "@/shared/api/modules";
import { buildSeoMetadata } from "@/shared/seo/metadata";
import { resolveSeoPageContextForRoute } from "@/shared/seo/runtime";
import {
  fetchPublishedPage,
  fetchPageArtifact,
  PageBuilderContent,
  DraftPreviewBanner,
} from "@rustok/pages-frontend";

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
    page = await fetchPublishedPage(slug, locale, tenantSlug, tenantId);
  } catch {
    // Fail gracefully for metadata generation
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
  const draft = await draftMode();
  const isDraft = draft.isEnabled;

  let page = null;
  try {
    page = await fetchPublishedPage(slug, locale, tenantSlug, tenantId);
  } catch (error) {
    console.error(`[StorefrontPage] Failed to fetch page "${slug}":`, error);
    notFound();
  }

  if (!page || !page.body) {
    notFound();
  }

  let artifact = { html: "", css: "" };
  try {
    artifact = await fetchPageArtifact(page.body.content, page.body.format, {
      slug,
      revalidate: isDraft ? false : 60,
      cache: isDraft ? "no-store" : undefined,
    });
  } catch (error) {
    console.error(`[StorefrontPage] Failed to fetch artifact for "${slug}":`, error);
    notFound();
  }

  return (
    <main className="rustok-page-wrapper min-h-screen">
      {isDraft && <DraftPreviewBanner slug={slug} locale={locale} />}
      <PageBuilderContent html={artifact.html} css={artifact.css} />
    </main>
  );
}
