import { setRequestConfig } from '@rustok/next-fluent/server';
import { headers } from 'next/headers';
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { acceptedLanguageRanges } from './accept-language';
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

/**
 * Applies the supported-locale allowlist to quality-ordered client
 * preferences. Field syntax and q-value ordering are owned by
 * `./accept-language`, which mirrors `rustok_ui_i18n::try_parse_accept_language`.
 */
function resolveAcceptLanguage(value: string | null): Locale | undefined {
  for (const range of acceptedLanguageRanges(value)) {
    const matched = matchSupportedLocale(range);
    if (matched) return matched;
  }
  return undefined;
}

/** Directory candidates are resolved once, without relying on CJS `__dirname`. */
function messageDirectories(): string[] {
  const candidates = [path.join(process.cwd(), 'messages')];
  try {
    const here = path.dirname(fileURLToPath(import.meta.url));
    candidates.push(path.resolve(here, '../../messages'));
    candidates.push(path.resolve(here, '../messages'));
  } catch {
    // Bundled CJS output has no `import.meta.url`; cwd is enough there.
  }
  return candidates;
}

const ftlCache = new Map<Locale, string>();

async function readFtlMessages(locale: Locale): Promise<string | undefined> {
  const cached = ftlCache.get(locale);
  if (cached !== undefined && process.env.NODE_ENV === 'production') {
    return cached;
  }

  for (const directory of messageDirectories()) {
    try {
      const content = await fs.readFile(
        /*turbopackIgnore: true*/ path.join(directory, `${locale}.ftl`),
        'utf8'
      );
      ftlCache.set(locale, content);
      return content;
    } catch {
      // try next candidate
    }
  }

  return undefined;
}

/**
 * Loads a locale catalog, falling back to the platform default locale.
 *
 * A missing default catalog is fatal: silently returning an empty catalog made
 * the admin render bare message keys with nothing but a `console.warn` to show
 * for it.
 */
async function loadFtlMessages(locale: Locale): Promise<string> {
  const requested = await readFtlMessages(locale);
  if (requested !== undefined) return requested;

  if (locale !== defaultLocale) {
    console.warn(
      `[next-admin] No FTL catalog for locale '${locale}'; falling back to '${defaultLocale}'.`
    );
    const fallback = await readFtlMessages(defaultLocale);
    if (fallback !== undefined) return fallback;
  }

  throw new Error(
    `[next-admin] Required FTL catalog '${defaultLocale}.ftl' was not found in ` +
      `${messageDirectories().join(', ')}`
  );
}

export default setRequestConfig(async (params) => {
  let locale: Locale | undefined = params?.locale
    ? matchSupportedLocale(params.locale)
    : undefined;

  if (!locale) {
    try {
      const headerStore = await headers();
      locale =
        matchSupportedLocale(headerStore.get(EFFECTIVE_LOCALE_HEADER)) ??
        resolveAcceptLanguage(headerStore.get('accept-language')) ??
        defaultLocale;
    } catch {
      locale = defaultLocale;
    }
  }

  const messages = await loadFtlMessages(locale);

  return {
    locale,
    messages
  };
});
