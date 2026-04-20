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
  const normalizedPath = path.startsWith("/") ? path : `/${path}`;
  if (normalizedPath === "/") {
    return `/${locale}`;
  }
  return `/${locale}${normalizedPath}`;
}

export { defaultLocale, locales };
