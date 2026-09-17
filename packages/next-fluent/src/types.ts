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

export type MessageArgsFor<K extends string, ArgsMap> =
  K extends keyof ArgsMap
    ? ArgsMap[K] extends Record<string, never> | undefined
      ? [args?: FluentArgs]
      : [args: ArgsMap[K]]
    : [args?: FluentArgs];

export interface TranslationFn<
  Key extends string = string,
  ArgsMap extends Record<string, any> = Record<string, any>
> {
  <K extends Key>(key: K, ...args: MessageArgsFor<K, ArgsMap>): string;
  raw<K extends Key>(key: K): string[] | string;
  rich<K extends Key>(key: K, values?: RichTranslationValues): React.ReactNode;
  has<K extends Key>(key: K): boolean;
}

export type Translations<
  Key extends string = string,
  ArgsMap extends Record<string, any> = Record<string, any>
> = TranslationFn<Key, ArgsMap>;

export interface FormattedMessageProps<
  Key extends string = string,
  ArgsMap extends Record<string, any> = Record<string, any>
> {
  id: Key;
  args?: Key extends keyof ArgsMap ? ArgsMap[Key] : FluentArgs;
  values?: RichTranslationValues;
  fallback?: React.ReactNode;
  className?: string;
  as?: React.ElementType;
}

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

