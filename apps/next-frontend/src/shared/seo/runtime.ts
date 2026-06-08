import { cache } from "react";
import type { MetadataRoute } from "next";

import { resolveLocale } from "../../i18n";
import { getStorefrontTenantSlug } from "@/shared/api/modules";

import type { SeoPageContext } from "./metadata";
import { defaultLocale, getSiteUrl, locales, localizedPath } from "./site";
import { fetchSeoPageContext, SeoTransportError } from "./transport";

const FALLBACK_API_BASE_URL =
  process.env.NEXT_PUBLIC_API_URL ??
  process.env.RUSTOK_API_URL ??
  "http://localhost:5150";

const MODULE_DISABLED_HINT = "seo module is not enabled";

type SeoRouteQueryPrimitive = string | number | boolean;

export type SeoRouteQueryInput = Record<
  string,
  SeoRouteQueryPrimitive | SeoRouteQueryPrimitive[] | null | undefined
>;

export type ResolveSeoRouteInput = {
  route?: string | null;
  routeSegment?: string | null;
  query?: SeoRouteQueryInput;
};

export type SeoPageContextResolution = {
  context: SeoPageContext | null;
  source: "rest" | "graphql" | "fallback_static";
  reason:
    | "ok"
    | "module_disabled"
    | "not_found"
    | "permission_denied"
    | "transport_failure";
  errorCode?: string;
};

function readBooleanFlag(
  primaryName: string,
  fallbackName: string,
  defaultValue: boolean,
): boolean {
  const raw = process.env[primaryName] ?? process.env[fallbackName];
  if (raw === undefined) {
    return defaultValue;
  }

  const normalized = raw.trim().toLowerCase();
  return (
    normalized === "1" ||
    normalized === "true" ||
    normalized === "yes" ||
    normalized === "on"
  );
}

function resolveApiBaseUrl(): string {
  return FALLBACK_API_BASE_URL.replace(/\/$/, "");
}

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
  const baseRoute = input.routeSegment
    ? `/modules/${input.routeSegment}`
    : normalizeRoutePath(input.route ?? "/");

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

function classifyNotFound(message: string): "module_disabled" | "not_found" {
  return message.toLowerCase().includes(MODULE_DISABLED_HINT)
    ? "module_disabled"
    : "not_found";
}

const resolveSeoPageContextCached = cache(
  async (
    locale: string,
    route: string,
    tenantSlug: string,
  ): Promise<SeoPageContextResolution> => {
    try {
      const result = await fetchSeoPageContext({
        locale,
        route,
        tenantSlug: tenantSlug || null,
        preferRest: true,
      });

      return {
        context: result.context,
        source: result.transport,
        reason: "ok",
      };
    } catch (error) {
      if (error instanceof SeoTransportError) {
        if (error.code === "NOT_FOUND") {
          return {
            context: null,
            source: "fallback_static",
            reason: classifyNotFound(error.message),
            errorCode: error.code,
          };
        }

        if (
          error.code === "PERMISSION_DENIED" ||
          error.code === "UNAUTHENTICATED"
        ) {
          return {
            context: null,
            source: "fallback_static",
            reason: "permission_denied",
            errorCode: error.code,
          };
        }

        return {
          context: null,
          source: "fallback_static",
          reason: "transport_failure",
          errorCode: error.code,
        };
      }

      return {
        context: null,
        source: "fallback_static",
        reason: "transport_failure",
        errorCode: "TRANSPORT_ERROR",
      };
    }
  },
);

export async function resolveSeoPageContextForRoute({
  locale,
  route,
  routeSegment,
  query,
}: {
  locale?: string | null;
  route?: string | null;
  routeSegment?: string | null;
  query?: SeoRouteQueryInput;
}): Promise<SeoPageContextResolution> {
  const resolvedLocale = resolveLocale(locale ?? defaultLocale);
  const tenantSlug = getStorefrontTenantSlug() ?? "";
  const normalizedRoute = buildDeterministicSeoRoute({
    route,
    routeSegment,
    query,
  });

  return resolveSeoPageContextCached(resolvedLocale, normalizedRoute, tenantSlug);
}

function staticRobotsMetadata(): MetadataRoute.Robots {
  return {
    rules: {
      userAgent: "*",
      allow: "/",
    },
    sitemap: `${getSiteUrl()}/sitemap.xml`,
  };
}

function staticSitemapMetadata(): MetadataRoute.Sitemap {
  const siteUrl = getSiteUrl();
  return locales.map((localeItem: string) => ({
    url: `${siteUrl}${localizedPath(localeItem, "/")}`,
  }));
}

type RobotsRuleAccumulator = {
  userAgent: string;
  allow: string[];
  disallow: string[];
};

function readRobotsFlag(): boolean {
  return readBooleanFlag(
    "NEXT_PUBLIC_SEO_NEXT_RUNTIME_SITEMAP_ENABLED",
    "SEO_NEXT_RUNTIME_SITEMAP_ENABLED",
    true,
  );
}

function parseRobotsRules(lines: string[]): RobotsRuleAccumulator[] {
  const rules: RobotsRuleAccumulator[] = [];
  let current: RobotsRuleAccumulator | null = null;

  const ensureCurrent = () => {
    if (!current) {
      current = {
        userAgent: "*",
        allow: [],
        disallow: [],
      };
      rules.push(current);
    }
    return current;
  };

  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) {
      continue;
    }

    const separator = line.indexOf(":");
    if (separator <= 0) {
      continue;
    }

    const directive = line.slice(0, separator).trim().toLowerCase();
    const value = line.slice(separator + 1).trim();

    if (!value) {
      continue;
    }

    if (directive === "user-agent") {
      current = {
        userAgent: value,
        allow: [],
        disallow: [],
      };
      rules.push(current);
      continue;
    }

    const target = ensureCurrent();
    if (directive === "allow") {
      target.allow.push(value);
      continue;
    }

    if (directive === "disallow") {
      target.disallow.push(value);
    }
  }

  return rules;
}

function parseRobotsMetadata(body: string): MetadataRoute.Robots {
  const lines = body.split(/\r?\n/);
  const rules = parseRobotsRules(lines)
    .filter((rule) => rule.allow.length > 0 || rule.disallow.length > 0)
    .map((rule) => ({
      userAgent: rule.userAgent,
      allow: rule.allow.length <= 1 ? rule.allow[0] : rule.allow,
      disallow:
        rule.disallow.length === 0
          ? undefined
          : rule.disallow.length === 1
            ? rule.disallow[0]
            : rule.disallow,
    }));

  const sitemapEntries = lines
    .map((line) => line.trim())
    .filter((line) => line.toLowerCase().startsWith("sitemap:"))
    .map((line) => line.slice("sitemap:".length).trim())
    .filter((line) => line.length > 0);

  return {
    rules:
      rules.length === 0
        ? staticRobotsMetadata().rules
        : rules.length === 1
          ? rules[0]
          : rules,
    sitemap:
      sitemapEntries.length === 0
        ? undefined
        : sitemapEntries.length === 1
          ? sitemapEntries[0]
          : sitemapEntries,
  };
}

function decodeXmlEntities(value: string): string {
  return value
    .replaceAll("&amp;", "&")
    .replaceAll("&lt;", "<")
    .replaceAll("&gt;", ">")
    .replaceAll("&quot;", '"')
    .replaceAll("&apos;", "'");
}

function extractXmlLocEntries(xml: string): string[] {
  const regex = /<loc>(.*?)<\/loc>/gims;
  const entries: string[] = [];

  for (const match of xml.matchAll(regex)) {
    const value = decodeXmlEntities(match[1]?.trim() ?? "");
    if (value.length > 0) {
      entries.push(value);
    }
  }

  return entries;
}

async function fetchSeoTextDocument(
  pathOrUrl: string,
  locale = defaultLocale,
): Promise<string> {
  const apiBaseUrl = resolveApiBaseUrl();
  const requestUrl =
    pathOrUrl.startsWith("http://") || pathOrUrl.startsWith("https://")
      ? pathOrUrl
      : new URL(pathOrUrl, apiBaseUrl).toString();

  const headers: Record<string, string> = {
    Accept: "text/plain, application/xml;q=0.9, */*;q=0.8",
    "Accept-Language": resolveLocale(locale),
  };

  const tenantSlug = getStorefrontTenantSlug();
  if (tenantSlug) {
    headers["X-Tenant-Slug"] = tenantSlug;
  }

  const response = await fetch(requestUrl, {
    method: "GET",
    headers,
    cache: "no-store",
  });

  if (!response.ok) {
    throw new Error(`SEO runtime fetch failed with ${response.status}`);
  }

  return response.text();
}

async function loadRuntimeSitemapUrls(): Promise<string[]> {
  const indexXml = await fetchSeoTextDocument("/sitemap.xml");
  const indexLocs = extractXmlLocEntries(indexXml);

  if (indexLocs.length === 0) {
    return [];
  }

  const pageUrls = new Set<string>();
  const sitemapFiles = indexLocs.filter((item) => item.toLowerCase().endsWith(".xml"));

  if (sitemapFiles.length === 0) {
    indexLocs.forEach((item) => pageUrls.add(item));
    return [...pageUrls].sort();
  }

  for (const sitemapFileUrl of sitemapFiles) {
    const fileXml = await fetchSeoTextDocument(sitemapFileUrl);
    const locs = extractXmlLocEntries(fileXml).filter(
      (item) => !item.toLowerCase().endsWith(".xml"),
    );
    for (const pageUrl of locs) {
      pageUrls.add(pageUrl);
    }
  }

  return [...pageUrls].sort();
}

export async function resolveRobotsMetadata(): Promise<MetadataRoute.Robots> {
  if (!readRobotsFlag()) {
    return staticRobotsMetadata();
  }

  try {
    const body = await fetchSeoTextDocument("/robots.txt");
    return parseRobotsMetadata(body);
  } catch {
    return staticRobotsMetadata();
  }
}

export async function resolveSitemapMetadata(): Promise<MetadataRoute.Sitemap> {
  if (!readRobotsFlag()) {
    return staticSitemapMetadata();
  }

  try {
    const urls = await loadRuntimeSitemapUrls();
    if (urls.length === 0) {
      return staticSitemapMetadata();
    }

    return urls.map((url) => ({
      url,
    }));
  } catch {
    return staticSitemapMetadata();
  }
}
