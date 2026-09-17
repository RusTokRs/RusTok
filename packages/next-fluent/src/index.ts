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
  FormattedMessage,
  useLocale,
  useTranslations,
  type FluentProviderProps,
  type FormattedMessageProps,
} from './client';

export {
  createFluentBundle,
  createTranslator,
  getCachedFluentBundle,
  clearBundleCache,
  getBundleCacheStats,
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
  buildKeyCandidates,
} from './utils';

export type {
  FluentBundle,
  FluentArgs,
  FluentVariable,
  NonEmptyArray,
  TagRenderFn,
  RichTranslationValues,
  MessageArgsFor,
  TranslationFn,
  Translations,
  RequestConfigFn,
  RequestConfigParams,
  RequestConfigResult,
  GetTranslationsOptions,
  I18nMiddlewareOptions,
  I18nConfig,
} from './types';


