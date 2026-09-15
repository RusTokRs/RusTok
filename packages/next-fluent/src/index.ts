export {
  FluentProvider,
  useLocale,
  useTranslations,
  type FluentProviderProps,
} from './client';

export {
  createFluentBundle,
  createTranslator,
} from './bundle';

export {
  normalizeLocaleTag,
  matchSupportedLocale,
  resolveAcceptLanguage,
  withKebabKey,
} from './utils';

export type {
  FluentArgs,
  FluentVariable,
  TranslationFn,
  Translations,
  RequestConfigFn,
  RequestConfigParams,
  RequestConfigResult,
  I18nMiddlewareOptions,
} from './types';
