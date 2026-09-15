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
  if (cached) return cached;

  const ftlPath = path.join(process.cwd(), "messages", `${locale}.ftl`);
  const content = await fs.readFile(ftlPath, "utf8");
  ftlCache.set(locale, content);
  return content;
}

export default setRequestConfig(async ({ locale }) => {
  const resolvedLocale = resolveLocale(locale);

  return {
    locale: resolvedLocale,
    messages: await loadFtlMessages(resolvedLocale),
  };
});
