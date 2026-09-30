import { defaultLocale, locales } from "../../i18n";

const FALLBACK_SITE_URL = "http://localhost:3000";

export function getSiteUrl(): string {
  const raw =
    process.env.NEXT_PUBLIC_SITE_URL ??
    process.env.NEXT_PUBLIC_STOREFRONT_URL ??
    FALLBACK_SITE_URL;
  return raw.replace(/\/$/, "");
}

export function localizedPath(locale: string, path = "/"): string {
  if (path.startsWith("http://") || path.startsWith("https://")) {
    return path;
  }

  const normalizedPath = path.startsWith("/") ? path : `/${path}`;
  // Backend SEO contexts may already return a locale-prefixed canonical path. Strip one
  // existing supported locale before applying the requested locale so fallback hreflang and
  // canonical generation never produce `/en/en/...` and can switch `/en/...` to `/de/...`.
  const pathWithoutLocale = locales.reduce((candidate, supportedLocale) => {
    const prefix = `/${supportedLocale}`;
    if (candidate === prefix) {
      return "/";
    }
    if (candidate.startsWith(`${prefix}/`)) {
      return candidate.slice(prefix.length) || "/";
    }
    return candidate;
  }, normalizedPath);

  if (pathWithoutLocale === "/") {
    return `/${locale}`;
  }
  return `/${locale}${pathWithoutLocale}`;
}

export { defaultLocale, locales };
