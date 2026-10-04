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
import { ArrowLeft, Calendar, MessageSquare, Tag } from "lucide-react";
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
  BlogCommentComposer,
} from "@rustok/blog-frontend";

interface BlogPostPageProps {
  params: Promise<{ locale: string; slug: string }>;
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

export default async function BlogPostPage({ params }: BlogPostPageProps) {
  const { locale, slug } = await params;
  const isRu = locale === "ru";
  const tenantSlug = getStorefrontTenantSlug();
  const tenantId = getStorefrontTenantId();

  if (!tenantId) {
    notFound();
  }

  let post;
  try {
    post = await fetchPublishedPost(
      storefrontGraphql,
      tenantId,
      tenantSlug,
      slug,
      locale
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

  return (
    <main className="min-h-screen bg-background">
      <article className="mx-auto max-w-4xl px-4 sm:px-6 py-10 space-y-8">
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
          <div className="flex flex-wrap items-center gap-3 text-xs text-muted-foreground">
            {publishedDate && (
              <span className="inline-flex items-center gap-1.5">
                <Calendar className="h-3.5 w-3.5" />
                <span>{publishedDate}</span>
              </span>
            )}
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
        <div className="prose prose-neutral dark:prose-invert max-w-none pt-2">
          <RichTextHtml
            view={post.content}
            contentLocale={post.effectiveLocale}
            className="text-base leading-8 text-foreground/90 space-y-4"
          />
        </div>

        {/* Comments Section */}
        <section className="mt-12 border-t border-border pt-8 space-y-6">
          <div className="flex items-center gap-2">
            <MessageSquare className="h-5 w-5 text-primary" />
            <h2 className="text-xl font-bold text-foreground">
              {isRu ? "Комментарии и обсуждение" : "Comments & Discussion"}
            </h2>
          </div>

          {/* Comment Composer */}
          {tenantSlug && (
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
          {post.publicComments.items.length === 0 ? (
            <div className="rounded-xl border border-dashed border-border p-6 text-center text-sm text-muted-foreground">
              {isRu
                ? "Пока нет комментариев. Оставьте отзыв первым!"
                : "No comments yet. Be the first to share your thoughts!"}
            </div>
          ) : (
            <div className="space-y-4">
              {post.publicComments.items.map((comment) => (
                <article
                  key={comment.id}
                  className="rounded-xl border border-border bg-card p-4 space-y-1.5 shadow-sm"
                >
                  <p className="whitespace-pre-line text-sm leading-6 text-foreground">
                    {comment.contentPreview}
                  </p>
                </article>
              ))}
            </div>
          )}
        </section>
      </article>
    </main>
  );
}
