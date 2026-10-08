/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

import { NextResponse } from "next/server";
import { storefrontGraphql } from "@/shared/lib/graphql";
import {
  getStorefrontTenantId,
  getStorefrontTenantSlug,
} from "@/shared/api/modules";
import { fetchPublishedPosts, type BlogPostSummary } from "@rustok/blog-frontend";

/**
 * Wraps text in a CDATA section. A literal `]]>` inside the text would close
 * the section early, so it is split across two sections.
 */
function cdata(value: string): string {
  return `<![CDATA[${value.split("]]>").join("]]]]><![CDATA[>")}]]>`;
}

export async function GET(
  _request: Request,
  { params }: { params: Promise<{ locale: string }> }
): Promise<NextResponse> {
  const { locale } = await params;
  const tenantSlug = getStorefrontTenantSlug();
  const tenantId = getStorefrontTenantId();

  let posts: BlogPostSummary[] = [];
  if (tenantId) {
    try {
      // The feed is locale-scoped: items use the requested locale with the
      // canonical fallback chain applied by the backend.
      const res = await fetchPublishedPosts(
        storefrontGraphql,
        tenantId,
        tenantSlug,
        null,
        25,
        undefined,
        undefined,
        locale
      );
      posts = res.items;
    } catch (err) {
      console.error("Failed to fetch blog posts for RSS feed:", err);
      return new NextResponse("Service Unavailable", {
        status: 503,
        headers: {
          "Content-Type": "text/plain; charset=utf-8",
          "Cache-Control": "no-store, no-cache, must-revalidate",
        },
      });
    }
  }

  const isRu = locale === "ru";
  const siteUrl = process.env.NEXT_PUBLIC_SITE_URL || "https://rustok.dev";
  const blogUrl = `${siteUrl}/${locale}/blog`;
  const feedTitle = isRu ? "Блог RusToK" : "RusToK Blog";
  const feedDescription = isRu
    ? "Последние новости, статьи и технические публикации платформы RusToK."
    : "Latest stories, engineering insights, and platform updates from RusToK.";

  const itemsXml = posts
    .map((post) => {
      const slugSegment = post.slug ? encodeURIComponent(post.slug) : post.id;
      const postUrl = `${blogUrl}/${slugSegment}`;
      const pubDate = post.publishedAt
        ? new Date(post.publishedAt).toUTCString()
        : new Date().toUTCString();
      const categoryTag = post.categoryName
        ? `\n      <category>${cdata(post.categoryName)}</category>`
        : "";
      const tagsXml = post.tags
        .map((tag) => `\n      <category>${cdata(tag)}</category>`)
        .join("");

      return `    <item>
      <title>${cdata(post.title)}</title>
      <link>${postUrl}</link>
      <guid isPermaLink="true">${postUrl}</guid>
      <pubDate>${pubDate}</pubDate>
      <description>${cdata(post.excerpt || post.title)}</description>${categoryTag}${tagsXml}
    </item>`;
    })
    .join("\n");

  const xml = `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom">
  <channel>
    <title>${cdata(feedTitle)}</title>
    <link>${blogUrl}</link>
    <description>${cdata(feedDescription)}</description>
    <language>${locale}</language>
    <lastBuildDate>${new Date().toUTCString()}</lastBuildDate>
    <atom:link href="${blogUrl}/feed.xml" rel="self" type="application/rss+xml"/>
${itemsXml}
  </channel>
</rss>`;

  return new NextResponse(xml, {
    status: 200,
    headers: {
      "Content-Type": "application/xml; charset=utf-8",
      "Cache-Control": "s-maxage=3600, stale-while-revalidate=86400",
    },
  });
}
