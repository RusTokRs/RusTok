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
import { getStorefrontTenantSlug } from "@/shared/api/modules";
import { BlogDraftPreview } from "@rustok/blog-frontend";

/**
 * Draft preview entry point for Blog preview-token links.
 *
 * The token lives in the URL fragment (`#token=…`), which only the browser can
 * read — so the rendering itself happens in the client island from the Blog
 * package. This server shell exists to own the route and, critically, to keep
 * the page out of search indexes: draft content must never be discovered by
 * crawlers regardless of the runtime SEO configuration.
 */
export const metadata: Metadata = {
  title: "Draft preview",
  robots: { index: false, follow: false, nocache: true },
};

interface BlogPreviewPageProps {
  params: Promise<{ locale: string }>;
}

export default async function BlogPreviewPage({
  params,
}: BlogPreviewPageProps) {
  const { locale } = await params;
  const tenantSlug = getStorefrontTenantSlug();

  return <BlogDraftPreview locale={locale} tenantSlug={tenantSlug} />;
}
