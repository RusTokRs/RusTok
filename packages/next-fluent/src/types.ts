import type { FluentBundle, FluentVariable } from '@fluent/bundle';

export type { FluentVariable };

export type FluentArgs = Record<string, FluentVariable>;

export interface TranslationFn {
  (key: string, args?: FluentArgs): string;
  raw(key: string): string[] | string;
}

export type Translations = TranslationFn;

export interface RequestConfigParams {
  locale?: string;
}

export interface RequestConfigResult {
  locale?: string;
  messages: string;
}

export type RequestConfigFn = (
  params: RequestConfigParams
) => Promise<RequestConfigResult> | RequestConfigResult;

export interface I18nMiddlewareOptions {
  locales: readonly string[];
  defaultLocale: string;
  localePrefix?: 'always' | 'as-needed' | 'never';
  cookieName?: string;
  headerName?: string;
}
