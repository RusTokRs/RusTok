import type { I18nConfig, Translations, GetTranslationsOptions } from './types';
import { createI18nMiddleware, type NextMiddlewareRequestLike } from './middleware';
import { forLocale, getLocale, setRequestConfig } from './server';
import { validateI18nConfig } from './utils';

export interface I18nRuntime {
  readonly config: I18nConfig;
  readonly middleware: (request: NextMiddlewareRequestLike) => Promise<any>;
  readonly proxy: (request: NextMiddlewareRequestLike) => Promise<any>;
  readonly getLocale: () => Promise<string>;
  readonly getTranslations: (options?: string | GetTranslationsOptions) => Promise<Translations>;
  readonly forLocale: (
    locale: string,
    options?: string | { namespace?: string; fallbackLocale?: string; fallbackLocales?: readonly string[]; debug?: boolean }
  ) => Promise<Translations>;
  readonly getMessages: (locale?: string) => Promise<string | readonly string[]>;
}

export function createI18n(config: I18nConfig): I18nRuntime {
  validateI18nConfig(config);

  const middlewareFn = createI18nMiddleware(config);

  if (config.loadMessages) {
    const loader = config.loadMessages;
    setRequestConfig(async ({ locale }) => {
      const target = locale ?? config.defaultLocale;
      const msgs = await loader(target);
      return {
        locale: target,
        messages: msgs,
      };
    });
  }

  const serverOptions = {
    locales: config.locales,
    defaultLocale: config.defaultLocale,
    headerName: config.headerName,
  };

  return {
    config,
    middleware: middlewareFn,
    proxy: middlewareFn,
    getLocale: () => getLocale(serverOptions),
    getTranslations: async (options?: string | GetTranslationsOptions) => {
      const explicitLocale = typeof options === 'object' && options ? options.locale : undefined;
      const locale = explicitLocale ?? (await getLocale(serverOptions));
      return forLocale(locale, options);
    },
    forLocale: (locale: string, options?: string | { namespace?: string; fallbackLocale?: string; fallbackLocales?: readonly string[]; debug?: boolean }) => {
      return forLocale(locale, options);
    },
    getMessages: async (locale?: string) => {
      const targetLocale = locale ?? (await getLocale(serverOptions));
      if (config.loadMessages) {
        return config.loadMessages(targetLocale);
      }
      return '';
    },
  };
}
