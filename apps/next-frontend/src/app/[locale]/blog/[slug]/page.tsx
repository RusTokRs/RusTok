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
import Image from "next/image";
import Link from "next/link";
import { notFound } from "next/navigation";
import { ArrowLeft, Calendar, Clock, MessageSquare, Tag } from "lucide-react";
import { storefrontGraphql } from "@/shared/lib/graphql";
import {
  getStorefrontTenantId,
  getStorefrontTenantSlug,
} from "@/shared/api/modules";
import { buildSeoMetadata } from "@/shared/seo/metadata";
import { resolveSeoPageContextForRoute } from "@/shared/seo/runtime";
import { RichTextHtml } from "@rustok/richtext/view";
import {
  fetchPublishedPost,
  fetchPublishedPosts,
  BlogCommentComposer,
  AuthorMiniBadge,
  AuthorBioCard,
  BlogTableOfContents,
  BlogShareButtons,
  CommentsPagination,
  PostCard,
  calculateReadingTime,
  formatReadingTime,
  buildArticleJsonLd,
  type BlogPostSummary,
} from "@rustok/blog-frontend";

interface BlogPostPageProps {
  params: Promise<{ locale: string; slug: string }>;
  searchParams?: Promise<Record<string, string | string[] | undefined>>;
}

export async function generateMetadata({
  params,
}: BlogPostPageProps): Promise<Metadata> {
  const { locale, slug } = await params;
  const tenantSlug = getStorefrontTenantSlug();
  const tenantId = getStorefrontTenantId();

  let post = null;
  if (tenantId) {
    try {
      post = await fetchPublishedPost(
        storefrontGraphql,
        tenantId,
        tenantSlug,
        slug,
        locale
      );
    } catch {
      post = null;
    }
  }

  const path = `/blog/${slug}`;
  const seoResolution = await resolveSeoPageContextForRoute({
    locale,
    route: path,
  });

  const title = post?.title || slug;
  const description = post?.excerpt || "Read this article on RusToK Blog.";

  return buildSeoMetadata({
    locale,
    title: `${title} | RusToK Blog`,
    description,
    path,
    context: seoResolution.context,
  });
}

export default async function BlogPostPage({
  params,
  searchParams,
}: BlogPostPageProps) {
  const { locale, slug } = await params;
  const query = (await searchParams) ?? {};
  const isRu = locale === "ru";
  const tenantSlug = getStorefrontTenantSlug();
  const tenantId = getStorefrontTenantId();

  if (!tenantId) {
    notFound();
  }

  const rawCommentsPage = typeof query.commentsPage === "string" ? parseInt(query.commentsPage, 10) : 1;
  const commentsPage = Number.isFinite(rawCommentsPage) && rawCommentsPage > 0 ? rawCommentsPage : 1;

  let post;
  try {
    post = await fetchPublishedPost(
      storefrontGraphql,
      tenantId,
      tenantSlug,
      slug,
      locale,
      commentsPage,
      20
    );
  } catch {
    notFound();
  }

  if (!post) {
    notFound();
  }

  const publishedDate = post.publishedAt
    ? new Date(post.publishedAt).toLocaleDateString(locale, {
        year: "numeric",
        month: "long",
        day: "numeric",
      })
    : null;

  const readingMinutes = calculateReadingTime(post.contentPlainText);
  const readingTimeLabel = formatReadingTime(readingMinutes, locale);

  let relatedPosts: BlogPostSummary[] = [];
  try {
    const relatedRes = await fetchPublishedPosts(
      storefrontGraphql,
      tenantId,
      tenantSlug,
      1,
      4
    );
    relatedPosts = relatedRes.items.filter((p) => p.slug !== slug).slice(0, 3);
  } catch {
    relatedPosts = [];
  }

  const articleUrl = `/${locale}/blog/${encodeURIComponent(slug)}`;
  const jsonLd = buildArticleJsonLd({
    post,
    url: articleUrl,
  });

  const comments = post.publicComments;
  const degradedCommentsMessage =
    comments.availability === "UNAVAILABLE"
      ? comments.cachedSnapshot
        ? isRu
          ? "Комментарии временно недоступны. Отображается сохраненная копия."
          : "Comments are temporarily unavailable. Showing a recent snapshot."
        : isRu
          ? "Комментарии временно недоступны. Статья доступна для чтения."
          : "Comments are temporarily unavailable. The article is still readable."
      : comments.availability === "TIMEOUT"
        ? comments.cachedSnapshot
          ? isRu
            ? "Истекло время ожидания комментариев. Отображается недавняя копия."
            : "Comments request timed out. Showing a recent cached snapshot."
          : isRu
            ? "Истекло время ожидания комментариев."
            : "Comments request timed out. The article is still available."
        : comments.availability === "READ_ONLY"
          ? isRu
            ? "Комментарии закрыты для новых ответов."
            : "Comments are closed for new replies."
          : null;

  return (
    <main className="min-h-screen bg-background">
      {/* Schema.org JSON-LD structured data for article */}
      <script
        type="application/ld+json"
        dangerouslySetInnerHTML={{ __html: JSON.stringify(jsonLd) }}
      />

      <article className="mx-auto max-w-6xl px-4 sm:px-6 py-10 space-y-8">
        {/* Navigation Breadcrumb */}
        <div>
          <Link
            href={`/${locale}/blog`}
            className="inline-flex items-center gap-1.5 text-xs font-medium text-muted-foreground hover:text-foreground transition"
          >
            <ArrowLeft className="h-3.5 w-3.5" />
            <span>{isRu ? "Назад ко всем статьям" : "Back to all articles"}</span>
          </Link>
        </div>

        {/* Article Header */}
        <header className="space-y-4 border-b border-border pb-6">
          <div className="flex flex-wrap items-center gap-4 text-xs text-muted-foreground">
            {post.authorProfile && (
              <AuthorMiniBadge author={post.authorProfile} locale={locale} />
            )}

            {publishedDate && (
              <span className="inline-flex items-center gap-1.5">
                <Calendar className="h-3.5 w-3.5" />
                <span>{publishedDate}</span>
              </span>
            )}

            <span className="inline-flex items-center gap-1.5">
              <Clock className="h-3.5 w-3.5" />
              <span>{readingTimeLabel}</span>
            </span>

            {post.tags.length > 0 && (
              <div className="flex flex-wrap items-center gap-1.5">
                <Tag className="h-3 w-3 text-muted-foreground" />
                {post.tags.map((tag) => (
                  <Link
                    key={tag}
                    href={`/${locale}/blog?tag=${encodeURIComponent(tag)}`}
                    className="rounded-full bg-primary/10 px-2.5 py-0.5 font-medium text-primary hover:bg-primary/20 transition text-[11px]"
                  >
                    #{tag}
                  </Link>
                ))}
              </div>
            )}
          </div>

          <h1 className="text-3xl font-extrabold tracking-tight text-foreground sm:text-4xl lg:text-5xl leading-tight">
            {post.title}
          </h1>

          {post.excerpt && (
            <p className="text-lg text-muted-foreground leading-relaxed">
              {post.excerpt}
            </p>
          )}
        </header>

        {/* 2-column layout: Article Content + Sticky ToC */}
        <div className="lg:grid lg:grid-cols-12 lg:gap-10 items-start">
          <div className="lg:col-span-8 space-y-8 min-w-0">
            {/* Featured Image */}
            {post.featuredImageUrl && (
              <div className="relative aspect-video w-full overflow-hidden rounded-2xl border border-border shadow-sm">
                <Image
                  src={post.featuredImageUrl}
                  alt={post.title}
                  fill
                  className="object-cover"
                  priority
                />
              </div>
            )}

            {/* Article Body */}
            <div
              id="article-body"
              className="prose prose-neutral dark:prose-invert max-w-none pt-2"
            >
              <RichTextHtml
                view={post.content}
                contentLocale={post.effectiveLocale}
                className="text-base leading-8 text-foreground/90 space-y-4"
              />
            </div>

            {/* Social Share Buttons */}
            <BlogShareButtons
              title={post.title}
              url={articleUrl}
              locale={locale}
            />

            {/* Author Bio Card */}
            {post.authorProfile && (
              <div className="pt-2">
                <AuthorBioCard author={post.authorProfile} locale={locale} />
              </div>
            )}

            {/* Related Articles */}
            {relatedPosts.length > 0 && (
              <section className="pt-8 border-t border-border space-y-4">
                <h3 className="text-xl font-bold text-foreground">
                  {isRu ? "Читайте также" : "Related Articles"}
                </h3>
                <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
                  {relatedPosts.map((related) => (
                    <PostCard
                      key={related.id}
                      post={related}
                      locale={locale}
                      href={
                        related.slug
                          ? `/${locale}/blog/${encodeURIComponent(related.slug)}`
                          : null
                      }
                    />
                  ))}
                </div>
              </section>
            )}

            {/* Comments Section */}
            <section id="comments" className="mt-12 border-t border-border pt-8 space-y-6">
              <div className="flex items-center justify-between gap-3">
                <div className="flex items-center gap-2">
                  <MessageSquare className="h-5 w-5 text-primary" />
                  <h2 className="text-xl font-bold text-foreground">
                    {isRu ? "Комментарии и обсуждение" : "Comments & Discussion"}
                  </h2>
                </div>
                {comments.total > 0 && (
                  <span className="text-xs text-muted-foreground font-medium">
                    {comments.total}{" "}
                    {isRu ? "комментариев" : "total comments"}
                  </span>
                )}
              </div>

              {/* Degraded comments notification */}
              {degradedCommentsMessage && (
                <div className="rounded-xl border border-border bg-muted/40 p-4 text-sm text-muted-foreground">
                  {degradedCommentsMessage}
                </div>
              )}

              {/* Comment Composer */}
              {tenantSlug && comments.availability === "AVAILABLE" && (
                <div className="rounded-2xl border border-border bg-card p-5 shadow-sm">
                  <BlogCommentComposer
                    tenantId={tenantId}
                    tenantSlug={tenantSlug}
                    postId={post.id}
                    contentLocale={post.effectiveLocale}
                  />
                </div>
              )}

              {/* Comments List */}
              {comments.items.length === 0 ? (
                <div className="rounded-xl border border-dashed border-border p-6 text-center text-sm text-muted-foreground">
                  {isRu
                    ? "Пока нет комментариев. Оставьте отзыв первым!"
                    : "No comments yet. Be the first to share your thoughts!"}
                </div>
              ) : (
                <div className="space-y-4">
                  {comments.items.map((comment) => (
                    <article
                      key={comment.id}
                      className="rounded-xl border border-border bg-card p-4 space-y-1.5 shadow-sm"
                    >
                      <p className="whitespace-pre-line text-sm leading-6 text-foreground">
                        {comment.contentPreview}
                      </p>
                      <div className="text-[11px] text-muted-foreground pt-1">
                        {new Date(comment.createdAt).toLocaleDateString(locale, {
                          year: "numeric",
                          month: "short",
                          day: "numeric",
                          hour: "2-digit",
                          minute: "2-digit",
                        })}
                      </div>
                    </article>
                  ))}

                  {/* Pagination */}
                  <CommentsPagination
                    currentPage={commentsPage}
                    totalItems={comments.total}
                    pageSize={20}
                    baseUrl={`/${locale}/blog/${encodeURIComponent(slug)}`}
                    locale={locale}
                  />
                </div>
              )}
            </section>
          </div>

          {/* Sidebar with Sticky Table of Contents */}
          <aside className="hidden lg:block lg:col-span-4 min-w-0">
            <BlogTableOfContents contentSelector="#article-body" locale={locale} />
          </aside>
        </div>
      </article>
    </main>
  );
}
