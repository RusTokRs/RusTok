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
import { BookOpen, Newspaper } from "lucide-react";
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

  return buildSeoMetadata({
    locale,
    title: isRu ? "Блог и новости | RusToK" : "Blog & Articles | RusToK",
    description: isRu
      ? "Читайте последние статьи, руководства и новости нашего проекта."
      : "Discover our latest insights, development stories, and community updates.",
    path,
    context: seoResolution.context,
  });
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
        pageSize
      );
      posts = res.items;
      totalPosts = res.total;
    }
  } catch {
    posts = [];
  }

  const filteredPosts = selectedTag
    ? posts.filter((p) => p.tags.includes(selectedTag))
    : posts;

  // Extract all distinct tags for filter pills
  const allTags = Array.from(new Set(posts.flatMap((p) => p.tags)));

  return (
    <main className="min-h-screen bg-background">
      <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8 py-10 space-y-8">
        {/* Header */}
        <div className="space-y-3 border-b border-border pb-6">
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

        {/* Tag Filters */}
        {allTags.length > 0 && (
          <div className="flex flex-wrap items-center gap-2">
            <Link
              href={`/${locale}/blog`}
              className={`rounded-full px-3 py-1 text-xs font-medium transition ${
                !selectedTag
                  ? "bg-primary text-primary-foreground shadow-sm"
                  : "bg-muted text-muted-foreground hover:bg-accent hover:text-foreground"
              }`}
            >
              {isRu ? "Все статьи" : "All Posts"}
            </Link>
            {allTags.map((tag) => {
              const active = selectedTag === tag;
              return (
                <Link
                  key={tag}
                  href={`/${locale}/blog?tag=${encodeURIComponent(tag)}`}
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
        )}

        {/* Posts Grid */}
        {filteredPosts.length === 0 ? (
          <div className="rounded-2xl border border-dashed border-border p-12 text-center">
            <Newspaper className="mx-auto h-10 w-10 text-muted-foreground/60 mb-3" />
            <h3 className="text-base font-semibold text-foreground">
              {isRu ? "Нет опубликованных статей" : "No articles found"}
            </h3>
            <p className="mt-1 text-sm text-muted-foreground">
              {isRu
                ? "В данный момент статьи отсутствуют или не соответствуют фильтру."
                : "Check back later or clear the selected tag filter."}
            </p>
          </div>
        ) : (
          <>
            <div className="grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
              {filteredPosts.map((post) => (
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
              totalItems={selectedTag ? filteredPosts.length : totalPosts}
              pageSize={pageSize}
              baseUrl={`/${locale}/blog`}
              selectedTag={selectedTag}
              locale={locale}
            />
          </>
        )}
      </div>
    </main>
  );
}
