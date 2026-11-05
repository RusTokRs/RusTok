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
import { storefrontGraphql } from "@/shared/lib/graphql";
import {
  getStorefrontTenantId,
  getStorefrontTenantSlug,
} from "@/shared/api/modules";
import { buildSeoMetadata } from "@/shared/seo/metadata";
import { resolveSeoPageContextForRoute } from "@/shared/seo/runtime";
import {
  fetchPublishedPosts,
  searchPublishedPosts,
  fetchBlogCategories,
  fetchBlogTags,
  BlogListingView,
  type BlogPostSummary,
} from "@rustok/blog-frontend";

interface BlogPageProps {
  params: Promise<{ locale: string }>;
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}

export async function generateMetadata({
  params,
}: {
  params: Promise<{ locale: string }>;
}): Promise<Metadata> {
  const { locale } = await params;
  const isRu = locale === "ru";
  const path = "/blog";
  const seoResolution = await resolveSeoPageContextForRoute({
    locale,
    route: path,
  });

  const metadata = buildSeoMetadata({
    locale,
    title: isRu ? "Блог и новости | RusToK" : "Blog & Articles | RusToK",
    description: isRu
      ? "Читайте последние статьи, руководства и новости нашего проекта."
      : "Discover our latest insights, development stories, and community updates.",
    path,
    context: seoResolution.context,
  });

  return {
    ...metadata,
    alternates: {
      ...metadata.alternates,
      types: {
        "application/rss+xml": `/${locale}/blog/feed.xml`,
      },
    },
  };
}

export default async function BlogListingPage({
  params,
  searchParams,
}: BlogPageProps) {
  const { locale } = await params;
  const query = await searchParams;
  const isRu = locale === "ru";
  const tenantSlug = getStorefrontTenantSlug();
  const tenantId = getStorefrontTenantId();

  const selectedTag = typeof query.tag === "string" ? query.tag.trim() : undefined;
  const selectedCategory = typeof query.category === "string" ? query.category.trim() : undefined;
  const searchQuery =
    typeof query.q === "string"
      ? query.q.trim()
      : typeof query.search === "string"
        ? query.search.trim()
        : undefined;

  const currentCursor =
    typeof query.after === "string" && query.after.trim() ? query.after.trim() : null;
  const pageSize = 9;

  // A full-text query uses Search's exact count and offset pagination.
  // The ordinary Blog feed retains its pinned-post keyset cursor, since
  // Search relevance offsets and Blog publication cursors are not interchangeable.
  const searchPage = Math.max(
    1,
    Math.min(100, Number.parseInt(typeof query.page === "string" ? query.page : "1", 10) || 1),
  );
  let posts: BlogPostSummary[] = [];
  let nextCursor: string | null = null;
  let searchTotal = 0;
  if (searchQuery) {
      // Search resolves tenant identity exclusively from the trusted host,
      // so a separate client-supplied tenant UUID is neither needed nor safe.
      const found = await searchPublishedPosts(
        storefrontGraphql,
        tenantSlug,
        searchQuery,
        locale,
        searchPage,
        pageSize,
        selectedCategory,
        selectedTag,
      );
      posts = found.items;
      searchTotal = found.total;
  } else if (tenantId) {
      const listed = await fetchPublishedPosts(
        storefrontGraphql,
        tenantId,
        tenantSlug,
        currentCursor,
        pageSize,
        selectedTag,
        selectedCategory,
        locale,
      );
      posts = listed.items;
      nextCursor = listed.nextCursor;
  }

  const filteredPosts = posts;
  const searchPageHref = (page: number) => {
    const params = new URLSearchParams();
    if (searchQuery) params.set("q", searchQuery);
    if (selectedCategory) params.set("category", selectedCategory);
    if (selectedTag) params.set("tag", selectedTag);
    if (page > 1) params.set("page", String(page));
    return `/${locale}/blog?${params.toString()}`;
  };

  // Load categories directly from taxonomy/blog GraphQL
  let allCategories: Array<{ id: string; name: string }> = [];
  try {
    const fetchedCategories = await fetchBlogCategories(
      storefrontGraphql,
      tenantId,
      tenantSlug,
      locale,
    );
    if (fetchedCategories.length > 0) {
      allCategories = fetchedCategories.map((c) => ({ id: c.id, name: c.name }));
    }
  } catch {
    // fallback
  }

  // Taxonomy owns the complete tag dictionary; a post page is not a facet API.
  let allTags: string[] = [];
  try {
    allTags = await fetchBlogTags(storefrontGraphql, tenantId, tenantSlug, locale);
  } catch {
    // Keep the listing accessible if taxonomy is unavailable; this fallback is
    // explicitly limited to tags present on the fetched page.
    allTags = Array.from(new Set(posts.flatMap((p) => p.tags)));
  }
  // Categories likewise fall back to their currently loaded post labels.
  if (allCategories.length === 0) {
    const categoriesMap = new Map<string, string>();
    for (const p of posts) {
      if (p.categoryId && p.categoryName) {
        categoriesMap.set(p.categoryId, p.categoryName);
      }
    }
    allCategories = Array.from(categoriesMap.entries()).map(([id, name]) => ({ id, name }));
  }

  return (
    <BlogListingView
      locale={locale}
      searchQuery={searchQuery}
      selectedCategory={selectedCategory}
      selectedTag={selectedTag}
      allCategories={allCategories}
      allTags={allTags}
      posts={filteredPosts}
      searchTotal={searchTotal}
      searchPage={searchPage}
      pageSize={pageSize}
      currentCursor={currentCursor}
      nextCursor={nextCursor}
      searchPageHref={searchPageHref}
    />
  );
}
