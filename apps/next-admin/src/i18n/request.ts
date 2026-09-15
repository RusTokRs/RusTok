import { setRequestConfig } from '@rustok/next-fluent/server';
import { headers } from 'next/headers';
import fs from 'node:fs/promises';
import path from 'node:path';
import {
  defaultLocale,
  EFFECTIVE_LOCALE_HEADER,
  locales,
  type Locale
} from './config';

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

const ftlCache = new Map<Locale, string>();

async function loadFtlMessages(locale: Locale): Promise<string> {
  const cached = ftlCache.get(locale);
  if (cached) return cached;

  const ftlPath = path.join(process.cwd(), 'messages', `${locale}.ftl`);
  const content = await fs.readFile(ftlPath, 'utf8');
  ftlCache.set(locale, content);
  return content;
}

export default setRequestConfig(async () => {
  let locale: Locale = defaultLocale;

  try {
    const headerStore = await headers();
    locale =
      matchSupportedLocale(headerStore.get(EFFECTIVE_LOCALE_HEADER)) ??
      resolveAcceptLanguage(headerStore.get('accept-language')) ??
      defaultLocale;
  } catch {
    // Fallback if called outside request context
  }

  const messages = await loadFtlMessages(locale);

  return {
    locale,
    messages
  };
});
