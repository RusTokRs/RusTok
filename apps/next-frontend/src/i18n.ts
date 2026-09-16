import { setRequestConfig } from "@rustok/next-fluent/server";
import fs from "node:fs/promises";
import path from "node:path";

export const locales = ["en", "ru"] as const;
export type Locale = (typeof locales)[number];
export const defaultLocale = "en";

function matchSupportedLocale(value?: string | null): Locale | undefined {
  const normalized = value?.trim().replaceAll("_", "-").toLowerCase();
  if (!normalized) return undefined;

  return (
    locales.find((locale) => locale.toLowerCase() === normalized) ??
    locales.find((locale) => locale.toLowerCase() === normalized.split("-")[0])
  );
}

export function resolveLocale(value?: string | null): Locale {
  return matchSupportedLocale(value) ?? defaultLocale;
}

const ftlCache = new Map<Locale, string>();

async function loadFtlMessages(locale: Locale): Promise<string> {
  const cached = ftlCache.get(locale);
  if (cached && process.env.NODE_ENV === "production") return cached;

  const candidatePaths = [
    path.join(process.cwd(), "messages", `${locale}.ftl`),
    path.resolve(process.cwd(), "apps/next-frontend/messages", `${locale}.ftl`),
  ];

  for (const candidate of candidatePaths) {
    try {
      const content = await fs.readFile(/*turbopackIgnore: true*/ candidate, "utf8");
      ftlCache.set(locale, content);
      return content;
    } catch {
      // try next candidate
    }
  }

  console.warn(`[next-frontend] Could not find FTL messages for locale: ${locale}`);
  return "";
}

export default setRequestConfig(async ({ locale }) => {
  const resolvedLocale = resolveLocale(locale);

  return {
    locale: resolvedLocale,
    messages: await loadFtlMessages(resolvedLocale),
  };
});
