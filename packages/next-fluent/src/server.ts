import { cache } from 'react';
import type { FluentBundle } from '@fluent/bundle';
import type { RequestConfigFn, Translations } from './types';
import { createFluentBundle, createTranslator } from './bundle';
import { resolveAcceptLanguage } from './utils';

let globalConfigFn: RequestConfigFn | null = null;

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

export async function getLocale(): Promise<string> {
  const store = getRequestStore();
  if (store.locale) {
    return store.locale;
  }

  try {
    const { headers, cookies } = await import('next/headers');
    const headerStore = await headers();
    const cookieStore = await cookies();

    // 1. Effective header
    const effectiveHeader = headerStore.get('x-rustok-effective-locale');
    if (effectiveHeader) {
      store.locale = effectiveHeader;
      return effectiveHeader;
    }

    // 2. Cookie
    const cookie =
      cookieStore.get('rustok-admin-locale')?.value ||
      cookieStore.get('rustok-frontend-locale')?.value ||
      cookieStore.get('NEXT_LOCALE')?.value;
    if (cookie) {
      store.locale = cookie;
      return cookie;
    }

    // 3. Accept-Language
    const acceptLang = headerStore.get('accept-language');
    if (acceptLang) {
      const resolved = resolveAcceptLanguage(acceptLang, ['en', 'ru']);
      if (resolved) {
        store.locale = resolved;
        return resolved;
      }
    }
  } catch {
    // If called outside request context (e.g. build time or test environment)
  }

  return 'en';
}

export async function getMessages(localeArg?: string): Promise<string> {
  const locale = localeArg ?? (await getLocale());
  if (globalConfigFn) {
    const res = await globalConfigFn({ locale });
    return res.messages;
  }
  return '';
}

export async function getTranslations(
  options?: string | { locale?: string; namespace?: string }
): Promise<Translations> {
  let namespace: string | undefined;
  let explicitLocale: string | undefined;

  if (typeof options === 'string') {
    namespace = options;
  } else if (options) {
    namespace = options.namespace;
    explicitLocale = options.locale;
  }

  const locale = explicitLocale ?? (await getLocale());
  const store = getRequestStore();

  let bundle = store.bundles.get(locale);
  if (!bundle) {
    const messages = await getMessages(locale);
    bundle = createFluentBundle(locale, messages);
    store.bundles.set(locale, bundle);
  }

  return createTranslator(bundle, namespace);
}
