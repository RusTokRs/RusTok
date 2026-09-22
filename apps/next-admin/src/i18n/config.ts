export const locales = ['en', 'ru'] as const;
export type Locale = (typeof locales)[number];

export const defaultLocale: Locale = 'en';
export const EFFECTIVE_LOCALE_HEADER = 'x-rustok-effective-locale';
export const LOCALE_COOKIE = 'rustok-admin-locale';

export const localeLabels: Record<Locale, string> = {
  en: 'English',
  ru: 'Русский'
};
