export {
  createI18n,
  type I18nRuntime,
} from './factory';

export {
  forLocale,
  getLocale,
  getTranslations,
  configureServerI18n,
  setRequestConfig,
  setRequestLocale,
} from './server';

export {
  createI18nMiddleware,
} from './middleware';

export {
  FluentProvider,
  useLocale,
  useTranslations,
  type FluentProviderProps,
} from './client';

export {
  createFluentBundle,
  createTranslator,
  type CreateFluentBundleOptions,
  type CreateTranslatorOptions,
} from './bundle';

export {
  createDefaultFunctions,
  unwrapFluentValue,
} from './functions';

export {
  parseRichText,
} from './rich';

export {
  pseudoLocalizeText,
  pseudoLocalizeFtl,
  type PseudoOptions,
} from './pseudo';

export {
  extractMessagesFromFtl,
  generateTypeDeclarations,
  type ExtractedMessage,
} from './typegen';

export {
  canonicalizeLocale,
  normalizeLocaleTag,
  matchSupportedLocale,
  resolveAcceptLanguage,
  validateI18nConfig,
  withKebabKey,
} from './utils';

export type {
  FluentBundle,
  FluentArgs,
  FluentVariable,
  NonEmptyArray,
  TagRenderFn,
  RichTranslationValues,
  TranslationFn,
  Translations,
  RequestConfigFn,
  RequestConfigParams,
  RequestConfigResult,
  GetTranslationsOptions,
  I18nMiddlewareOptions,
  I18nConfig,
} from './types';


