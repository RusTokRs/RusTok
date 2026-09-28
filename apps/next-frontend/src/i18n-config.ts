export const locales = ["en", "ru"] as const;
export type Locale = (typeof locales)[number];
export const defaultLocale = "en";

export function matchSupportedLocale(value?: string | null): Locale | undefined {
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
