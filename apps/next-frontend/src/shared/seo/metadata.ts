import type { Metadata } from "next";

import { defaultLocale, getSiteUrl, localizedPath, locales } from "./site";

export type SeoAlternateLink = {
  locale: string;
  href: string;
  xDefault?: boolean;
};

export type SeoRedirectDecision = {
  targetUrl: string;
  statusCode: number;
};

export type SeoRouteContext = {
  targetKind?: string | null;
  targetId?: string | null;
  requestedLocale?: string | null;
  effectiveLocale: string;
  canonicalUrl: string;
  redirect?: SeoRedirectDecision | null;
  alternates?: SeoAlternateLink[] | null;
};

export type SeoImageAsset = {
  url: string;
  alt?: string | null;
  width?: number | null;
  height?: number | null;
  mimeType?: string | null;
};

export type SeoOpenGraph = {
  title?: string | null;
  description?: string | null;
  kind?: string | null;
  siteName?: string | null;
  url?: string | null;
  locale?: string | null;
  images?: SeoImageAsset[] | null;
};

export type SeoTwitterCard = {
  card?: string | null;
  title?: string | null;
  description?: string | null;
  site?: string | null;
  creator?: string | null;
  images?: SeoImageAsset[] | null;
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

export type SeoPagination = {
  prevUrl?: string | null;
  nextUrl?: string | null;
};

export type SeoStructuredDataBlock = {
  id?: string | null;
  schemaKind?: string | null;
  schemaType?: string | null;
  kind?: string | null;
  source?: "explicit" | "generated" | "fallback" | string | null;
  payload: unknown;
};

export type SeoMetaTag = {
  name?: string | null;
  property?: string | null;
  httpEquiv?: string | null;
  content: string;
};

export type SeoLinkTag = {
  rel: string;
  href: string;
  hreflang?: string | null;
  media?: string | null;
  mimeType?: string | null;
  title?: string | null;
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

export type SeoDocument = {
  title: string;
  description?: string | null;
  robots: SeoRobots;
  openGraph?: SeoOpenGraph | null;
  twitter?: SeoTwitterCard | null;
  verification?: SeoVerification | null;
  pagination?: SeoPagination | null;
  structuredDataBlocks?: SeoStructuredDataBlock[] | null;
  metaTags?: SeoMetaTag[] | null;
  linkTags?: SeoLinkTag[] | null;
};

export type SeoPageContext = {
  route: SeoRouteContext;
  document: SeoDocument;
};

export type SeoStructuredDataScript = {
  key: string;
  json: string;
};

type BuildSeoMetadataOptions = {
  locale?: string;
  title?: string;
  description?: string;
  path?: string;
  context?: SeoPageContext | null;
};

function isAbsoluteUrl(pathOrUrl: string): boolean {
  return (
    pathOrUrl.startsWith("http://") ||
    pathOrUrl.startsWith("https://") ||
    pathOrUrl.startsWith("//")
  );
}

function toAbsoluteUrl(pathOrUrl: string): string {
  return new URL(pathOrUrl, getSiteUrl()).toString();
}

function toRoutePath(pathOrUrl: string): string {
  if (!isAbsoluteUrl(pathOrUrl)) {
    return pathOrUrl;
  }

  const parsed = new URL(pathOrUrl);
  return `${parsed.pathname}${parsed.search}${parsed.hash}`;
}

function normalizeRobots(robots?: SeoRobots | null): Metadata["robots"] {
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

function buildAlternates(
  locale: string,
  canonicalUrl: string,
  alternates?: SeoAlternateLink[] | null,
): Metadata["alternates"] {
  const canonicalPath = toRoutePath(canonicalUrl);
  const fallbackLanguages = Object.fromEntries(
    locales.map((item: string) => [
      item,
      toAbsoluteUrl(localizedPath(item, canonicalPath)),
    ]),
  );

  if (!alternates || alternates.length === 0) {
    return {
      canonical: toAbsoluteUrl(localizedPath(locale, canonicalPath)),
      languages: fallbackLanguages,
    };
  }

  const languages = Object.fromEntries(
    alternates
      .filter((item) => item.locale !== "x-default")
      .map((item) => [item.locale, toAbsoluteUrl(item.href)]),
  );
  const xDefault = alternates.find((item) => item.locale === "x-default");

  return {
    canonical: toAbsoluteUrl(canonicalUrl),
    languages: {
      ...fallbackLanguages,
      ...languages,
      ...(xDefault ? { "x-default": toAbsoluteUrl(xDefault.href) } : {}),
    },
  };
}

function buildOpenGraph(
  locale: string,
  canonicalUrl: string,
  openGraph?: SeoOpenGraph | null,
  fallbackTitle?: string,
  fallbackDescription?: string,
): Metadata["openGraph"] {
  if (!openGraph && !fallbackTitle && !fallbackDescription) {
    return undefined;
  }

  return {
    type: openGraph?.kind === "article" ? "article" : "website",
    title: openGraph?.title || fallbackTitle,
    description: openGraph?.description || fallbackDescription,
    siteName: openGraph?.siteName || undefined,
    url: toAbsoluteUrl(openGraph?.url || canonicalUrl),
    locale: openGraph?.locale || locale,
    images: openGraph?.images?.map((item) => ({
      url: toAbsoluteUrl(item.url),
      alt: item.alt ?? undefined,
      width: item.width ?? undefined,
      height: item.height ?? undefined,
      type: item.mimeType ?? undefined,
    })),
  };
}

function buildTwitter(
  twitter?: SeoTwitterCard | null,
  fallbackTitle?: string,
  fallbackDescription?: string,
): Metadata["twitter"] {
  if (!twitter && !fallbackTitle && !fallbackDescription) {
    return undefined;
  }

  return {
    card: twitter?.card === "summary" ? "summary" : "summary_large_image",
    title: twitter?.title || fallbackTitle,
    description: twitter?.description || fallbackDescription,
    site: twitter?.site || undefined,
    creator: twitter?.creator || undefined,
    images: twitter?.images?.map((item) => toAbsoluteUrl(item.url)),
  };
}

function buildVerification(
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

function serializeStructuredData(payload: unknown): string | null {
  if (payload === null || payload === undefined) {
    return null;
  }

  if (typeof payload !== "object") {
    return null;
  }

  try {
    return JSON.stringify(payload);
  } catch {
    return null;
  }
}

export function buildSeoStructuredDataScripts(
  context?: SeoPageContext | null,
): SeoStructuredDataScript[] {
  if (!context?.document.structuredDataBlocks?.length) {
    return [];
  }

  return context.document.structuredDataBlocks
    .map((block, index) => {
      const json = serializeStructuredData(block.payload);
      if (!json) {
        return null;
      }

      return {
        key:
          block.id?.trim() ||
          block.schemaKind?.trim() ||
          block.schemaType?.trim() ||
          `seo-structured-data-${index}`,
        json,
      };
    })
    .filter((item): item is SeoStructuredDataScript => item !== null);
}

export function buildSeoMetadata({
  locale = defaultLocale,
  title = "RusToK Storefront",
  description = "Next.js storefront for RusToK",
  path = "/",
  context,
}: BuildSeoMetadataOptions = {}): Metadata {
  const effectiveLocale = context?.route.effectiveLocale || locale;
  const canonicalUrl =
    context?.route.canonicalUrl || localizedPath(effectiveLocale, path);
  const documentTitle = context?.document.title || title;
  const documentDescription = context?.document.description || description;

  return {
    metadataBase: new URL(getSiteUrl()),
    title: documentTitle,
    description: documentDescription,
    alternates: buildAlternates(
      effectiveLocale,
      canonicalUrl,
      context?.route.alternates,
    ),
    robots: normalizeRobots(context?.document.robots),
    openGraph: buildOpenGraph(
      effectiveLocale,
      canonicalUrl,
      context?.document.openGraph,
      documentTitle,
      documentDescription,
    ),
    twitter: buildTwitter(
      context?.document.twitter,
      documentTitle,
      documentDescription,
    ),
    verification: buildVerification(context?.document.verification),
  };
}
