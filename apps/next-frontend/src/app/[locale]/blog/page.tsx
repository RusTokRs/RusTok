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
import Link from "next/link";
import { BookOpen, Newspaper, Rss } from "lucide-react";
import { storefrontGraphql } from "@/shared/lib/graphql";
import {
  getStorefrontTenantId,
  getStorefrontTenantSlug,
} from "@/shared/api/modules";
import { buildSeoMetadata } from "@/shared/seo/metadata";
import { resolveSeoPageContextForRoute } from "@/shared/seo/runtime";
import {
  fetchPublishedPosts,
  PostCard,
  BlogPagination,
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
  const rawPage = typeof query.page === "string" ? parseInt(query.page, 10) : 1;
  const page = Number.isFinite(rawPage) && rawPage > 0 ? rawPage : 1;
  const pageSize = 9;

  let posts: BlogPostSummary[] = [];
  let totalPosts = 0;
  try {
    if (tenantId) {
      const res = await fetchPublishedPosts(
        storefrontGraphql,
        tenantId,
        tenantSlug,
        page,
        pageSize,
        selectedTag,
        selectedCategory,
      );
      posts = res.items;
      totalPosts = res.total;
    }
  } catch {
    posts = [];
  }

  // Extract all distinct tags and categories from results
  const allTags = Array.from(new Set(posts.flatMap((p) => p.tags)));
  const categoriesMap = new Map<string, string>();
  for (const p of posts) {
    if (p.categoryId && p.categoryName) {
      categoriesMap.set(p.categoryId, p.categoryName);
    }
  }
  const allCategories = Array.from(categoriesMap.entries()).map(([id, name]) => ({ id, name }));

  return (
    <main className="min-h-screen bg-background">
      <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8 py-10 space-y-8">
        {/* Header */}
        <div className="flex flex-col sm:flex-row sm:items-end sm:justify-between gap-4 border-b border-border pb-6">
          <div className="space-y-3">
            <div className="flex items-center gap-2 text-primary text-xs font-semibold uppercase tracking-wider">
              <BookOpen className="h-4 w-4" />
              <span>{isRu ? "Блог проекта" : "Project Blog"}</span>
            </div>
            <h1 className="text-3xl font-bold tracking-tight text-foreground sm:text-4xl">
              {isRu ? "Статьи и анонсы" : "Stories & Announcements"}
            </h1>
            <p className="text-sm text-muted-foreground max-w-2xl">
              {isRu
                ? "Актуальные публикации, технические обзоры и обновления платформы RusToK."
                : "Read about platform updates, engineering notes, and best practices."}
            </p>
          </div>

          <Link
            href={`/${locale}/blog/feed.xml`}
            target="_blank"
            className="inline-flex items-center gap-1.5 self-start sm:self-auto rounded-full border border-border bg-card px-3 py-1.5 text-xs font-medium text-muted-foreground hover:border-primary/50 hover:text-foreground transition-colors shadow-sm"
            title={isRu ? "RSS-лента блога" : "Blog RSS Feed"}
          >
            <Rss className="h-3.5 w-3.5 text-orange-500" />
            <span>RSS</span>
          </Link>
        </div>

        {/* Category & Tag Filters */}
        <div className="space-y-3">
          {/* Categories row */}
          {allCategories.length > 0 && (
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-xs font-semibold text-muted-foreground mr-1">
                {isRu ? "Рубрики:" : "Categories:"}
              </span>
              <Link
                href={`/${locale}/blog${selectedTag ? `?tag=${encodeURIComponent(selectedTag)}` : ""}`}
                className={`rounded-full px-3 py-1 text-xs font-medium transition ${
                  !selectedCategory
                    ? "bg-secondary text-secondary-foreground shadow-sm ring-1 ring-border"
                    : "bg-muted text-muted-foreground hover:bg-accent hover:text-foreground"
                }`}
              >
                {isRu ? "Все рубрики" : "All Categories"}
              </Link>
              {allCategories.map((cat) => {
                const active = selectedCategory === cat.id;
                return (
                  <Link
                    key={cat.id}
                    href={`/${locale}/blog?category=${encodeURIComponent(cat.id)}${selectedTag ? `&tag=${encodeURIComponent(selectedTag)}` : ""}`}
                    className={`rounded-full px-3 py-1 text-xs font-medium transition ${
                      active
                        ? "bg-secondary text-secondary-foreground shadow-sm ring-2 ring-primary"
                        : "bg-muted text-muted-foreground hover:bg-accent hover:text-foreground"
                    }`}
                  >
                    {cat.name}
                  </Link>
                );
              })}
            </div>
          )}

          {/* Tag Filters */}
          <div className="flex flex-wrap items-center gap-2">
            <Link
              href={`/${locale}/blog`}
              className={`rounded-full px-3 py-1 text-xs font-medium transition ${
                !selectedTag && !selectedCategory
                  ? "bg-primary text-primary-foreground shadow-sm"
                  : "bg-muted text-muted-foreground hover:bg-accent hover:text-foreground"
              }`}
            >
              {isRu ? "Все статьи" : "All Posts"}
            </Link>
            {selectedTag && !allTags.includes(selectedTag) && (
              <Link
                href={`/${locale}/blog?tag=${encodeURIComponent(selectedTag)}${selectedCategory ? `&category=${encodeURIComponent(selectedCategory)}` : ""}`}
                className="rounded-full bg-primary text-primary-foreground px-3 py-1 text-xs font-medium shadow-sm transition"
              >
                #{selectedTag}
              </Link>
            )}
            {allTags.map((tag) => {
              const active = selectedTag === tag;
              return (
                <Link
                  key={tag}
                  href={`/${locale}/blog?tag=${encodeURIComponent(tag)}${selectedCategory ? `&category=${encodeURIComponent(selectedCategory)}` : ""}`}
                  className={`rounded-full px-3 py-1 text-xs font-medium transition ${
                    active
                      ? "bg-primary text-primary-foreground shadow-sm"
                      : "bg-muted text-muted-foreground hover:bg-accent hover:text-foreground"
                  }`}
                >
                  #{tag}
                </Link>
              );
            })}
          </div>

          {/* Active Filters row */}
          {(selectedCategory || selectedTag) && (
            <div className="flex flex-wrap items-center gap-2 pt-2 border-t border-border/50 text-xs">
              <span className="text-muted-foreground font-medium">
                {isRu ? "Активные фильтры:" : "Active filters:"}
              </span>
              {selectedCategory && (
                <span className="inline-flex items-center gap-1 rounded-full bg-secondary px-2.5 py-0.5 font-medium text-secondary-foreground">
                  <span>{isRu ? "Рубрика" : "Category"}</span>
                  <Link
                    href={`/${locale}/blog${selectedTag ? `?tag=${encodeURIComponent(selectedTag)}` : ""}`}
                    className="ml-1 font-bold text-muted-foreground hover:text-foreground"
                    title={isRu ? "Убрать фильтр по рубрике" : "Remove category filter"}
                  >
                    ×
                  </Link>
                </span>
              )}
              {selectedTag && (
                <span className="inline-flex items-center gap-1 rounded-full bg-primary/10 px-2.5 py-0.5 font-medium text-primary">
                  <span>#{selectedTag}</span>
                  <Link
                    href={`/${locale}/blog${selectedCategory ? `?category=${encodeURIComponent(selectedCategory)}` : ""}`}
                    className="ml-1 font-bold text-muted-foreground hover:text-foreground"
                    title={isRu ? "Убрать фильтр по тегу" : "Remove tag filter"}
                  >
                    ×
                  </Link>
                </span>
              )}
              <Link
                href={`/${locale}/blog`}
                className="ml-auto text-xs text-muted-foreground hover:text-foreground underline"
              >
                {isRu ? "Сбросить все" : "Clear all"}
              </Link>
            </div>
          )}
        </div>

        {/* Posts Grid */}
        {posts.length === 0 ? (
          <div className="rounded-2xl border border-dashed border-border p-12 text-center">
            <Newspaper className="mx-auto h-10 w-10 text-muted-foreground/60 mb-3" />
            <h3 className="text-base font-semibold text-foreground">
              {isRu ? "Нет опубликованных статей" : "No articles found"}
            </h3>
            <p className="mt-1 text-sm text-muted-foreground">
              {isRu
                ? "В данный момент статьи отсутствуют или не соответствуют фильтру."
                : "Check back later or clear the selected filter."}
            </p>
          </div>
        ) : (
          <>
            <div className="grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
              {posts.map((post) => (
                <PostCard
                  key={post.id}
                  post={post}
                  locale={locale}
                  href={
                    post.slug
                      ? `/${locale}/blog/${encodeURIComponent(post.slug)}`
                      : null
                  }
                />
              ))}
            </div>

            {/* Pagination */}
            <BlogPagination
              currentPage={page}
              totalItems={totalPosts}
              pageSize={pageSize}
              baseUrl={`/${locale}/blog`}
              selectedTag={selectedTag}
              selectedCategory={selectedCategory}
              locale={locale}
            />
          </>
        )}
      </div>
    </main>
  );
}
