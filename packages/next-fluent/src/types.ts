import type { FluentBundle, FluentVariable } from '@fluent/bundle';
import type React from 'react';

export type { FluentBundle, FluentVariable };

export type NonEmptyArray<T> = readonly [T, ...T[]];

export type FluentArgs = Record<string, FluentVariable>;

export type TagRenderFn = (children: React.ReactNode) => React.ReactNode;

export type RichTranslationValues = Record<
  string,
  FluentVariable | TagRenderFn | React.ReactNode
>;

export interface TranslationFn<Key extends string = string> {
  (key: Key, args?: FluentArgs): string;
  raw(key: Key): string[] | string;
  rich(key: Key, values?: RichTranslationValues): React.ReactNode;
  has(key: Key): boolean;
}

export type Translations<Key extends string = string> = TranslationFn<Key>;

export interface RequestConfigParams {
  locale?: string;
}

export interface RequestConfigResult {
  locale?: string;
  messages: string | readonly string[];
  fallbackLocale?: string;
  fallbackMessages?: string | readonly string[];
}

export type RequestConfigFn = (
  params: RequestConfigParams
) => Promise<RequestConfigResult> | RequestConfigResult;

export interface GetTranslationsOptions {
  locale?: string;
  messages?: string | readonly string[];
  fallbackLocale?: string;
  fallbackLocales?: readonly string[];
  fallbackMessages?: string | readonly string[];
  namespace?: string;
  debug?: boolean;
}

export interface I18nMiddlewareOptions {
  locales: readonly string[];
  defaultLocale: string;
  localePrefix?: 'always' | 'as-needed' | 'never';
  cookieName?: string;
  headerName?: string;
}

export interface I18nConfig {
  locales: readonly string[];
  defaultLocale: string;
  localePrefix?: 'always' | 'as-needed' | 'never';
  cookieName?: string;
  headerName?: string;
  loadMessages?: (locale: string) => Promise<string | readonly string[]> | string | readonly string[];
}

