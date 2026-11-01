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

export type SeoRouteQueryPrimitive = string | number | boolean;

export type SeoRouteQueryInput = Record<
  string,
  SeoRouteQueryPrimitive | SeoRouteQueryPrimitive[] | null | undefined
>;

export type ResolveSeoRouteInput = {
  route?: string | null;
  routeSegment?: string | null;
  query?: SeoRouteQueryInput;
};

export type SeoVerificationTag = {
  name: string;
  value: string;
};

export type SeoVerification = {
  google?: string[] | null;
  yandex?: string[] | null;
  yahoo?: string[] | null;
  other?: SeoVerificationTag[] | null;
};

export type SeoRobots = {
  index: boolean;
  follow: boolean;
  noarchive?: boolean;
  nosnippet?: boolean;
  noimageindex?: boolean;
  notranslate?: boolean;
  maxSnippet?: number | null;
  maxImagePreview?: string | null;
  maxVideoPreview?: number | null;
  custom?: string[] | null;
};

function normalizeQueryValue(
  value: SeoRouteQueryPrimitive | SeoRouteQueryPrimitive[] | null | undefined,
): string | null {
  if (Array.isArray(value)) {
    for (const item of value) {
      const normalized = normalizeQueryValue(item);
      if (normalized) {
        return normalized;
      }
    }
    return null;
  }

  if (value === null || value === undefined) {
    return null;
  }

  const normalized = `${value}`.trim();
  return normalized.length > 0 ? normalized : null;
}

function normalizeRoutePath(route: string): string {
  const normalized = route.trim();
  if (!normalized) {
    return "/";
  }
  if (normalized.startsWith("/")) {
    return normalized;
  }
  return `/${normalized}`;
}

export function buildDeterministicSeoRoute(input: ResolveSeoRouteInput): string {
  if (input.routeSegment) {
    const baseRoute = `/modules/${input.routeSegment}`;
    const params = new URLSearchParams();
    Object.entries(input.query ?? {})
      .filter(([key]) => key !== "lang")
      .sort(([left], [right]) => left.localeCompare(right))
      .forEach(([key, value]) => {
        const normalizedValue = normalizeQueryValue(value);
        if (normalizedValue) {
          params.set(key, normalizedValue);
        }
      });

    const serialized = params.toString();
    return serialized ? `${baseRoute}?${serialized}` : baseRoute;
  }

  const rawRoute = normalizeRoutePath(input.route ?? "/");
  const productMatch = rawRoute.match(/^(?:\/([a-z]{2}(?:-[a-z]{2})?))?\/products\/([^/?#]+)/i);
  if (productMatch) {
    const slug = productMatch[2];
    return `/modules/product?handle=${encodeURIComponent(slug)}&slug=${encodeURIComponent(slug)}`;
  }

  const blogMatch = rawRoute.match(/^(?:\/([a-z]{2}(?:-[a-z]{2})?))?\/blog\/([^/?#]+)/i);
  if (blogMatch) {
    const slug = blogMatch[2];
    return `/modules/blog?slug=${encodeURIComponent(slug)}`;
  }

  const pageMatch = rawRoute.match(/^(?:\/([a-z]{2}(?:-[a-z]{2})?))?\/pages\/([^/?#]+)/i);
  if (pageMatch) {
    const slug = pageMatch[2];
    return `/modules/pages?slug=${encodeURIComponent(slug)}`;
  }

  const params = new URLSearchParams();
  Object.entries(input.query ?? {})
    .filter(([key]) => key !== "lang")
    .sort(([left], [right]) => left.localeCompare(right))
    .forEach(([key, value]) => {
      const normalizedValue = normalizeQueryValue(value);
      if (normalizedValue) {
        params.set(key, normalizedValue);
      }
    });

  const serialized = params.toString();
  return serialized ? `${rawRoute}?${serialized}` : rawRoute;
}

export function serializeStructuredData(payload: unknown): string | null {
  if (payload === null || payload === undefined) {
    return null;
  }

  if (typeof payload !== "object") {
    return null;
  }

  try {
    return JSON.stringify(payload)
      .replace(/</g, "\\u003c")
      .replace(/>/g, "\\u003e")
      .replace(/&/g, "\\u0026")
      .replace(/\u2028/g, "\\u2028")
      .replace(/\u2029/g, "\\u2029");
  } catch {
    return null;
  }
}

export function normalizeRobots(robots?: SeoRobots | null): Metadata["robots"] {
  if (!robots) {
    return { index: true, follow: true };
  }

  const maxImagePreview =
    robots.maxImagePreview === "none" ||
    robots.maxImagePreview === "standard" ||
    robots.maxImagePreview === "large"
      ? robots.maxImagePreview
      : undefined;

  return {
    index: robots.index,
    follow: robots.follow,
    noarchive: robots.noarchive,
    nosnippet: robots.nosnippet,
    noimageindex: robots.noimageindex,
    notranslate: robots.notranslate,
    "max-snippet": robots.maxSnippet ?? undefined,
    "max-image-preview": maxImagePreview,
    "max-video-preview": robots.maxVideoPreview ?? undefined,
  };
}

export function buildVerification(
  verification?: SeoVerification | null,
): Metadata["verification"] {
  if (!verification) {
    return undefined;
  }

  const otherEntries = (verification.other ?? [])
    .filter((item) => item.name.trim() !== "" && item.value.trim() !== "")
    .map((item) => [item.name, item.value] as const);

  return {
    google:
      verification.google && verification.google.length > 0
        ? verification.google
        : undefined,
    yandex:
      verification.yandex && verification.yandex.length > 0
        ? verification.yandex
        : undefined,
    yahoo:
      verification.yahoo && verification.yahoo.length > 0
        ? verification.yahoo
        : undefined,
    other: otherEntries.length > 0 ? Object.fromEntries(otherEntries) : undefined,
  };
}

