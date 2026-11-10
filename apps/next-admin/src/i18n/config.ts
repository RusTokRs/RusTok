export const locales = ['en', 'ru'] as const;
export type Locale = (typeof locales)[number];

export const defaultLocale: Locale = 'en';
export const EFFECTIVE_LOCALE_HEADER = 'x-rustok-effective-locale';
export const LOCALE_COOKIE = 'rustok-admin-locale';

export const localeLabels: Record<Locale, string> = {
  en: 'English',
  ru: 'Русский'
};

export function matchSupportedLocale(value?: string | null): Locale | undefined {
  const normalized = value?.trim().replaceAll('_', '-').toLowerCase();
  if (!normalized) return undefined;

  return (
    locales.find((locale) => locale.toLowerCase() === normalized) ??
    locales.find((locale) => locale.toLowerCase() === normalized.split('-')[0])
  );
}

export function resolveLocale(value?: string | null): Locale {
  return matchSupportedLocale(value) ?? defaultLocale;
}

