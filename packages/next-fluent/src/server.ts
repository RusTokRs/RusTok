import { cache } from 'react';
import type { FluentBundle } from '@fluent/bundle';
import type { RequestConfigFn, Translations } from './types';
import { createFluentBundle, createTranslator } from './bundle';
import {
  matchConfiguredLocaleIdentity,
  matchSupportedLocale,
  resolveAcceptLanguage,
  validateI18nConfig,
} from './utils';

let globalConfigFn: RequestConfigFn | null = null;
let globalLocales: readonly string[] = ['en', 'ru'];
let globalDefaultLocale: string = 'en';

export interface ServerI18nOptions {
  locales?: readonly string[];
  defaultLocale?: string;
  cookieNames?: readonly string[];
  headerName?: string;
}

export function configureServerI18n(config: {
  locales?: readonly string[];
  defaultLocale?: string;
}): void {
  const nextLocales =
    config.locales && config.locales.length > 0 ? config.locales : globalLocales;
  const nextDefaultLocale = config.defaultLocale ?? globalDefaultLocale;

  // Validate the prospective pair before mutating either global. Partial updates
  // therefore cannot leave a default locale outside the configured locale set.
  validateI18nConfig({
    locales: nextLocales,
    defaultLocale: nextDefaultLocale,
  });
  const configuredDefaultLocale = matchConfiguredLocaleIdentity(
    nextDefaultLocale,
    nextLocales
  );
  if (!configuredDefaultLocale) {
    throw new Error('[next-fluent] Unable to resolve configured default locale identity.');
  }

  globalLocales = nextLocales;
  globalDefaultLocale = configuredDefaultLocale;
}

export function setRequestConfig(fn: RequestConfigFn): RequestConfigFn {
  globalConfigFn = fn;
  return fn;
}

export function getRequestConfig(): RequestConfigFn | null {
  return globalConfigFn;
}

interface RequestStore {
  locale?: string;
  bundles: Map<string, FluentBundle>;
}

export const getRequestStore = cache((): RequestStore => ({
  bundles: new Map(),
}));

export function setRequestLocale(locale: string): void {
  getRequestStore().locale = locale;
}

export async function getLocale(options?: ServerI18nOptions): Promise<string> {
  const store = getRequestStore();
  if (store.locale) {
    return store.locale;
  }

  const allowedLocales = options?.locales ?? globalLocales;
  const requestedDefaultLocale = options?.defaultLocale ?? globalDefaultLocale;
  const defLocale = matchConfiguredLocaleIdentity(requestedDefaultLocale, allowedLocales);
  if (!defLocale) {
    throw new Error(
      '[next-fluent] "defaultLocale" must resolve to an exact configured locale identity.'
    );
  }
  const headerKey = options?.headerName ?? 'x-rustok-effective-locale';
  const cookieList = options?.cookieNames ?? [
    'rustok-locale',
    'rustok-admin-locale',
    'rustok-frontend-locale',
    'NEXT_LOCALE',
  ];

  try {
    const { headers, cookies } = await import('next/headers');
    const headerStore = await headers();
    const cookieStore = await cookies();

    // 1. Effective header with allow-list validation
    const rawHeader = headerStore.get(headerKey);
    const validHeaderLocale = matchSupportedLocale(rawHeader, allowedLocales);
    if (validHeaderLocale) {
      store.locale = validHeaderLocale;
      return validHeaderLocale;
    }

    // 2. Cookies with allow-list validation
    for (const cName of cookieList) {
      const cVal = cookieStore.get(cName)?.value;
      const validCookieLocale = matchSupportedLocale(cVal, allowedLocales);
      if (validCookieLocale) {
        store.locale = validCookieLocale;
        return validCookieLocale;
      }
    }

    // 3. Accept-Language with allow-list negotiation
    const acceptLang = headerStore.get('accept-language');
    if (acceptLang) {
      const resolved = resolveAcceptLanguage(acceptLang, allowedLocales);
      if (resolved) {
        store.locale = resolved;
        return resolved;
      }
    }
  } catch {
    // If called outside request context (e.g. build time or test environment)
  }

  store.locale = defLocale;
  return defLocale;
}

export async function getMessages(
  localeArg?: string
): Promise<string | readonly string[]> {
  const locale = localeArg ?? (await getLocale());
  if (globalConfigFn) {
    const res = await globalConfigFn({ locale });
    return res.messages;
  }
  return '';
}

export interface ForLocaleOptions {
  messages?: string | readonly string[];
  fallbackLocale?: string;
  fallbackLocales?: readonly string[];
  fallbackMessages?: string | readonly string[];
  namespace?: string;
  debug?: boolean;
}

export async function forLocale(
  locale: string,
  options?: string | ForLocaleOptions
): Promise<Translations> {
  let namespace: string | undefined;
  let fallbackLocale: string | undefined;
  let fallbackLocales: readonly string[] | undefined;
  let fallbackMessages: string | readonly string[] | undefined;
  let explicitMessages: string | readonly string[] | undefined;
  let debug = false;

  if (typeof options === 'string') {
    namespace = options;
  } else if (options) {
    namespace = options.namespace;
    fallbackLocale = options.fallbackLocale;
    fallbackLocales = options.fallbackLocales;
    fallbackMessages = options.fallbackMessages;
    explicitMessages = options.messages;
    debug = options.debug ?? false;
  }

  let bundle: FluentBundle;
  if (explicitMessages) {
    bundle = createFluentBundle(locale, explicitMessages);
  } else {
    const store = getRequestStore();
    let cached = store.bundles.get(locale);
    if (!cached) {
      const messages = await getMessages(locale);
      cached = createFluentBundle(locale, messages);
      store.bundles.set(locale, cached);
    }
    bundle = cached;
  }

  const fallbackBundleList: FluentBundle[] = [];

  // 1. Explicit fallback messages
  if (fallbackMessages) {
    const fbLoc = fallbackLocale ?? 'en';
    fallbackBundleList.push(createFluentBundle(fbLoc, fallbackMessages));
  }

  // 2. Fallback locales from request/config
  const fallbacksToLoad = new Set<string>();
  if (fallbackLocale && fallbackLocale !== locale && !fallbackMessages) {
    fallbacksToLoad.add(fallbackLocale);
  }
  if (fallbackLocales) {
    for (const fb of fallbackLocales) {
      if (fb && fb !== locale) fallbacksToLoad.add(fb);
    }
  }

  if (fallbacksToLoad.size > 0) {
    const store = getRequestStore();
    for (const fbLocale of fallbacksToLoad) {
      let fbBundle = store.bundles.get(fbLocale);
      if (!fbBundle) {
        const fbMessages = await getMessages(fbLocale);
        fbBundle = createFluentBundle(fbLocale, fbMessages);
        store.bundles.set(fbLocale, fbBundle);
      }
      fallbackBundleList.push(fbBundle);
    }
  }

  return createTranslator(bundle, {
    fallbackBundles: fallbackBundleList,
    namespace,
    debug,
  });
}

export async function getTranslations(
  options?: string | ({ locale?: string } & ForLocaleOptions)
): Promise<Translations> {
  const explicitLocale = typeof options === 'object' && options ? options.locale : undefined;
  const locale = explicitLocale ?? (await getLocale());
  return forLocale(locale, options);
}
