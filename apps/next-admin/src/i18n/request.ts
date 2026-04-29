import { getRequestConfig } from 'next-intl/server';
import { headers } from 'next/headers';

const messageLoaders = {
  en: () => import('../../messages/en.json').then((module) => module.default),
  ru: () => import('../../messages/ru.json').then((module) => module.default)
} as const;

export type Locale = keyof typeof messageLoaders;
export const locales = Object.keys(messageLoaders) as Locale[];
export const defaultLocale: Locale = 'en';

export const EFFECTIVE_LOCALE_HEADER = 'x-rustok-effective-locale';

function matchSupportedLocale(value?: string | null): Locale | undefined {
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

function resolveAcceptLanguage(value: string | null): Locale | undefined {
  return value
    ?.split(',')
    .map((item) => item.split(';')[0]?.trim())
    .filter(Boolean)
    .map((item) => matchSupportedLocale(item))
    .find((locale): locale is Locale => Boolean(locale));
}

export default getRequestConfig(async () => {
  const headerStore = await headers();
  const locale =
    matchSupportedLocale(headerStore.get(EFFECTIVE_LOCALE_HEADER)) ??
    resolveAcceptLanguage(headerStore.get('accept-language')) ??
    defaultLocale;

  return {
    locale,
    messages: await messageLoaders[locale]()
  };
});
