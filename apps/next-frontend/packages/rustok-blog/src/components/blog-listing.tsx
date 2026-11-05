/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import { getTranslations } from '@rustok/next-fluent/server';
import React from 'react';
import Link from 'next/link';
import { BookOpen, Newspaper, Rss, Search } from 'lucide-react';
import type { BlogPostSummary } from '../api/posts';
import { PostCard } from './post-card';
import { BlogPagination } from './blog-pagination';

export interface BlogListingViewProps {
  locale: string;
  searchQuery?: string;
  selectedCategory?: string;
  selectedTag?: string;
  allCategories: Array<{ id: string; name: string }>;
  allTags: string[];
  posts: BlogPostSummary[];
  searchTotal: number;
  searchPage: number;
  pageSize: number;
  currentCursor: string | null;
  nextCursor: string | null;
  searchPageHref: (page: number) => string;
}

export async function BlogListingView({
  locale,
  searchQuery,
  selectedCategory,
  selectedTag,
  allCategories,
  allTags,
  posts,
  searchTotal,
  searchPage,
  pageSize,
  currentCursor,
  nextCursor,
  searchPageHref,
}: BlogListingViewProps) {
  const t = await getTranslations('Blog');
  return (
    <main className="min-h-screen bg-background">
      <div className="mx-auto max-w-7xl px-4 sm:px-6 lg:px-8 py-10 space-y-8">
        {/* Header */}
        <div className="flex flex-col sm:flex-row sm:items-end sm:justify-between gap-4 border-b border-border pb-6">
          <div className="space-y-3">
            <div className="flex items-center gap-2 text-primary text-xs font-semibold uppercase tracking-wider">
              <BookOpen className="h-4 w-4" />
              <span>{t('listing.badge')}</span>
            </div>
            <h1 className="text-3xl font-bold tracking-tight text-foreground sm:text-4xl">
              {t('listing.title')}
            </h1>
            <p className="text-sm text-muted-foreground max-w-2xl">
              {t('listing.description')}
            </p>
          </div>

          <Link
            href={`/${locale}/blog/feed.xml`}
            target="_blank"
            className="inline-flex items-center gap-1.5 self-start sm:self-auto rounded-full border border-border bg-card px-3 py-1.5 text-xs font-medium text-muted-foreground hover:border-primary/50 hover:text-foreground transition-colors shadow-sm"
            title={t('listing.feed')}
          >
            <Rss className="h-3.5 w-3.5 text-orange-500" />
            <span>RSS</span>
          </Link>
        </div>

        {/* Search & Filters */}
        <div className="space-y-4">
          {/* Search bar */}
          <form
            method="GET"
            action={`/${locale}/blog`}
            className="relative max-w-md w-full"
          >
            {selectedCategory && (
              <input type="hidden" name="category" value={selectedCategory} />
            )}
            {selectedTag && (
              <input type="hidden" name="tag" value={selectedTag} />
            )}
            <div className="relative">
              <Search className="absolute left-3.5 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground pointer-events-none" />
              <input
                type="search"
                name="q"
                defaultValue={searchQuery ?? ''}
                placeholder={t('listing.search')}
                className="w-full rounded-full border border-border bg-card pl-10 pr-4 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-primary/50 shadow-sm"
              />
            </div>
          </form>

          {/* Categories row */}
          {allCategories.length > 0 && (
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-xs font-semibold text-muted-foreground mr-1">
                {t('listing.categories')}
              </span>
              <Link
                href={`/${locale}/blog${selectedTag ? `?tag=${encodeURIComponent(selectedTag)}` : ''}${searchQuery ? `${selectedTag ? '&' : '?'}q=${encodeURIComponent(searchQuery)}` : ''}`}
                className={`rounded-full px-3 py-1 text-xs font-medium transition ${
                  !selectedCategory
                    ? 'bg-secondary text-secondary-foreground shadow-sm ring-1 ring-border'
                    : 'bg-muted text-muted-foreground hover:bg-accent hover:text-foreground'
                }`}
              >
                {t('listing.allCategories')}
              </Link>
              {allCategories.map((cat) => {
                const active = selectedCategory === cat.id;
                return (
                  <Link
                    key={cat.id}
                    href={`/${locale}/blog?category=${encodeURIComponent(cat.id)}${selectedTag ? `&tag=${encodeURIComponent(selectedTag)}` : ''}${searchQuery ? `&q=${encodeURIComponent(searchQuery)}` : ''}`}
                    className={`rounded-full px-3 py-1 text-xs font-medium transition ${
                      active
                        ? 'bg-secondary text-secondary-foreground shadow-sm ring-2 ring-primary'
                        : 'bg-muted text-muted-foreground hover:bg-accent hover:text-foreground'
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
              href={`/${locale}/blog${searchQuery ? `?q=${encodeURIComponent(searchQuery)}` : ''}`}
              className={`rounded-full px-3 py-1 text-xs font-medium transition ${
                !selectedTag && !selectedCategory
                  ? 'bg-primary text-primary-foreground shadow-sm'
                  : 'bg-muted text-muted-foreground hover:bg-accent hover:text-foreground'
              }`}
            >
              {t('listing.allPosts')}
            </Link>
            {selectedTag && !allTags.includes(selectedTag) && (
              <Link
                href={`/${locale}/blog?tag=${encodeURIComponent(selectedTag)}${selectedCategory ? `&category=${encodeURIComponent(selectedCategory)}` : ''}${searchQuery ? `&q=${encodeURIComponent(searchQuery)}` : ''}`}
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
                  href={`/${locale}/blog?tag=${encodeURIComponent(tag)}${selectedCategory ? `&category=${encodeURIComponent(selectedCategory)}` : ''}${searchQuery ? `&q=${encodeURIComponent(searchQuery)}` : ''}`}
                  className={`rounded-full px-3 py-1 text-xs font-medium transition ${
                    active
                      ? 'bg-primary text-primary-foreground shadow-sm'
                      : 'bg-muted text-muted-foreground hover:bg-accent hover:text-foreground'
                  }`}
                >
                  #{tag}
                </Link>
              );
            })}
          </div>

          {/* Active Filters row */}
          {(selectedCategory || selectedTag || searchQuery) && (
            <div className="flex flex-wrap items-center gap-2 pt-2 border-t border-border/50 text-xs">
              <span className="text-muted-foreground font-medium">
                {t('listing.activeFilters')}
              </span>
              {selectedCategory && (
                <span className="inline-flex items-center gap-1 rounded-full bg-secondary px-2.5 py-0.5 font-medium text-secondary-foreground">
                  <span>{t('listing.category')}</span>
                  <Link
                    href={`/${locale}/blog${selectedTag ? `?tag=${encodeURIComponent(selectedTag)}` : ''}${searchQuery ? `${selectedTag ? '&' : '?'}q=${encodeURIComponent(searchQuery)}` : ''}`}
                    className="ml-1 font-bold text-muted-foreground hover:text-foreground"
                    title={t('listing.removeCategory')}
                  >
                    ×
                  </Link>
                </span>
              )}
              {selectedTag && (
                <span className="inline-flex items-center gap-1 rounded-full bg-primary/10 px-2.5 py-0.5 font-medium text-primary">
                  <span>#{selectedTag}</span>
                  <Link
                    href={`/${locale}/blog${selectedCategory ? `?category=${encodeURIComponent(selectedCategory)}` : ''}${searchQuery ? `${selectedCategory ? '&' : '?'}q=${encodeURIComponent(searchQuery)}` : ''}`}
                    className="ml-1 font-bold text-muted-foreground hover:text-foreground"
                    title={t('listing.removeTag')}
                  >
                    ×
                  </Link>
                </span>
              )}
              {searchQuery && (
                <span className="inline-flex items-center gap-1 rounded-full bg-primary/10 px-2.5 py-0.5 font-medium text-primary">
                  <span>{t('listing.searchLabel', { searchQuery: searchQuery })}</span>
                  <Link
                    href={`/${locale}/blog${selectedCategory ? `?category=${encodeURIComponent(selectedCategory)}` : ''}${selectedTag ? `${selectedCategory ? '&' : '?'}tag=${encodeURIComponent(selectedTag)}` : ''}`}
                    className="ml-1 font-bold text-muted-foreground hover:text-foreground"
                    title={t('listing.clearSearch')}
                  >
                    ×
                  </Link>
                </span>
              )}
              <Link
                href={`/${locale}/blog`}
                className="ml-auto text-xs text-muted-foreground hover:text-foreground underline"
              >
                {t('listing.clearFilters')}
              </Link>
            </div>
          )}
        </div>

        {/* Posts Grid */}
        {posts.length === 0 ? (
          <div className="rounded-2xl border border-dashed border-border p-12 text-center">
            <Newspaper className="mx-auto h-10 w-10 text-muted-foreground/60 mb-3" />
            <h3 className="text-base font-semibold text-foreground">
              {searchQuery
                ? t('listing.emptyQuery', { searchQuery: searchQuery })
                : t('listing.emptyTitle')}
            </h3>
            <p className="mt-1 text-sm text-muted-foreground">
              {searchQuery
                ? t('listing.emptySearch')
                : t('listing.emptyBody')}
            </p>
            {searchQuery && searchTotal > 0 && searchPage > 1 && (
              <Link
                href={searchPageHref(1)}
                className="mt-4 inline-flex rounded-full border px-4 py-2 text-sm"
              >
                {t('listing.firstResults')}
              </Link>
            )}
            {(searchQuery || selectedCategory || selectedTag) && (
              <div className="mt-4">
                <Link
                  href={`/${locale}/blog`}
                  className="inline-flex items-center justify-center rounded-xl bg-primary px-4 py-2 text-xs font-semibold text-primary-foreground hover:bg-primary/90 transition shadow-sm"
                >
                  {t('listing.resetFilters')}
                </Link>
              </div>
            )}
          </div>
        ) : (
          <>
            <div className="grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
              {posts.map((post) => (
                <PostCard
                  key={post.id}
                  post={post}
                  locale={locale}
                  filters={{
                    tag: selectedTag,
                    category: selectedCategory,
                    q: searchQuery,
                  }}
                  href={
                    post.slug
                      ? `/${locale}/blog/${encodeURIComponent(post.slug)}`
                      : null
                  }
                />
              ))}
            </div>

            {/* Full-text results use relevance-offset pagination; the feed
                retains Blog's independent keyset cursor. */}
            {searchQuery ? (
              <nav
                aria-label={t('listing.searchPages')}
                className="flex items-center justify-center gap-4 py-6"
              >
                {searchPage > 1 && (
                  <Link
                    href={searchPageHref(searchPage - 1)}
                    className="rounded-full border px-4 py-2 text-sm"
                  >
                    {t('pagination.previous')}
                  </Link>
                )}
                <span className="text-sm text-muted-foreground">
                  {t('listing.searchResults', { searchPage: searchPage, searchTotal: searchTotal })}
                </span>
                {searchPage * pageSize < searchTotal && (
                  <Link
                    href={searchPageHref(searchPage + 1)}
                    className="rounded-full border px-4 py-2 text-sm"
                  >
                    {t('pagination.next')}
                  </Link>
                )}
              </nav>
            ) : (
              <BlogPagination
                nextCursor={nextCursor}
                currentCursor={currentCursor}
                baseUrl={`/${locale}/blog`}
                selectedTag={selectedTag}
                selectedCategory={selectedCategory}
                searchQuery={undefined}
                locale={locale}
              />
            )}
          </>
        )}
      </div>
    </main>
  );
}
