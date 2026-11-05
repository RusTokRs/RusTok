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
import { notFound, permanentRedirect } from "next/navigation";
import { storefrontGraphql } from "@/shared/lib/graphql";
import {
  getStorefrontTenantId,
  getStorefrontTenantSlug,
} from "@/shared/api/modules";
import { buildSeoMetadata } from "@/shared/seo/metadata";
import { getSiteUrl } from "@/shared/seo/site";
import { resolveSeoPageContextForRoute } from "@/shared/seo/runtime";
import {
  fetchPublishedPost,
  fetchPublishedPosts,
  BlogPostDetailView,
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

  // Metadata must not mistake a failed owner read for a missing article.
  // A retired slug resolves to the live post, whose current slug owns the
  // canonical metadata route just as it does the permanent page redirect.
  const post = tenantId
    ? await fetchPublishedPost(
        storefrontGraphql,
        tenantId,
        tenantSlug,
        slug,
        locale
      )
    : null;

  const path = `/blog/${encodeURIComponent(post?.slug ?? slug)}`;
  const seoResolution = await resolveSeoPageContextForRoute({
    locale,
    route: path,
  });

  // Explicit SEO fields win; otherwise use the localized post content.
  // The default description is localized, never a hard-coded English string
  // for non-English pages.
  const title = post?.seoTitle || post?.title || slug;
  const defaultDescription =
    locale === "ru"
      ? "Читайте эту статью в блоге RusToK."
      : "Read this article on RusToK Blog.";
  const description = post?.seoDescription || post?.excerpt || defaultDescription;

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

  // A failed GraphQL request is not a missing article. Let the route error
  // boundary report backend failure; only an actual null post is a 404.
  const post = await fetchPublishedPost(
    storefrontGraphql,
    tenantId,
    tenantSlug,
    slug,
    locale,
    commentsPage,
    20
  );

  if (!post) {
    notFound();
  }

  // The post was resolved through a retired slug (or the slug was otherwise
  // non-canonical): redirect permanently to the current canonical slug.
  if (post.slug && post.slug !== slug) {
    permanentRedirect(`/${locale}/blog/${encodeURIComponent(post.slug)}`);
  }

  // Related articles: same category first (when the post has one), then the
  // most recent posts to fill up to three. Both reads use the requested locale.
  let relatedPosts: BlogPostSummary[] = [];
  try {
    const collected: BlogPostSummary[] = [];
    if (post.categoryId) {
      const sameCategory = await fetchPublishedPosts(
        storefrontGraphql,
        tenantId,
        tenantSlug,
        null,
        4,
        undefined,
        post.categoryId,
        locale
      );
      collected.push(...sameCategory.items);
    }
    if (collected.filter((p) => p.slug !== post.slug).length < 3) {
      const recent = await fetchPublishedPosts(
        storefrontGraphql,
        tenantId,
        tenantSlug,
        null,
        4,
        undefined,
        undefined,
        locale
      );
      collected.push(...recent.items);
    }
    const seen = new Set<string>([post.id]);
    relatedPosts = collected
      .filter((item) => {
        if (seen.has(item.id) || item.slug === post.slug) return false;
        seen.add(item.id);
        return true;
      })
      .slice(0, 3);
  } catch {
    relatedPosts = [];
  }

  return (
    <BlogPostDetailView
      post={post}
      locale={locale}
      slug={slug}
      tenantSlug={tenantSlug}
      tenantId={tenantId}
      commentsPage={commentsPage}
      relatedPosts={relatedPosts}
      siteUrl={getSiteUrl()}
    />
  );
}
