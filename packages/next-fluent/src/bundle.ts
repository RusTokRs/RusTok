import { FluentBundle, FluentResource, type FluentFunction } from '@fluent/bundle';
import type React from 'react';
import type { FluentArgs, FluentVariable, RichTranslationValues, Translations } from './types';
import { buildKeyCandidates } from './utils';
import { parseRichText } from './rich';
import { createDefaultFunctions } from './functions';

export interface CreateFluentBundleOptions {
  useIsolating?: boolean;
  functions?: Record<string, FluentFunction>;
}

export function createFluentBundle(
  locale: string,
  ftlSource: string | readonly string[],
  options: CreateFluentBundleOptions = {}
): FluentBundle {
  const defaultFunctions = createDefaultFunctions(locale);
  const bundle = new FluentBundle(locale, {
    // Keep Project Fluent's bidi safety enabled by default. Consumers that need
    // byte-for-byte legacy output can opt out explicitly, but normal UI rendering
    // must isolate interpolated values so mixed LTR/RTL text remains well ordered.
    useIsolating: options.useIsolating ?? true,
    functions: {
      ...defaultFunctions,
      ...options.functions,
    },
  });
  const sources = Array.isArray(ftlSource) ? ftlSource : [ftlSource];
  for (const src of sources) {
    if (!src || typeof src !== 'string') continue;
    const resource = new FluentResource(src);
    const errors = bundle.addResource(resource, { allowOverrides: true });
    if (errors && errors.length > 0) {
      console.warn(`[next-fluent] Warnings adding FTL resource for locale ${locale}:`, errors);
    }
  }
  return bundle;
}

export interface CreateTranslatorOptions {
  fallbackBundle?: FluentBundle | null;
  fallbackBundles?: FluentBundle | readonly FluentBundle[] | null;
  namespace?: string;
  debug?: boolean;
}

const FORMAT_ERROR = Symbol('format-error');
type FormatCandidateResult = string | null | typeof FORMAT_ERROR;

export function createTranslator(
  bundle: FluentBundle | null,
  namespaceOrFallbackOrOpts?: string | FluentBundle | CreateTranslatorOptions | null,
  maybeNamespace?: string
): Translations {
  const allBundles: FluentBundle[] = [];
  if (bundle) allBundles.push(bundle);

  let namespace: string | undefined;
  let debug = false;

  if (typeof namespaceOrFallbackOrOpts === 'string') {
    namespace = namespaceOrFallbackOrOpts;
  } else if (namespaceOrFallbackOrOpts && typeof namespaceOrFallbackOrOpts === 'object') {
    if ('locales' in namespaceOrFallbackOrOpts) {
      const fb = namespaceOrFallbackOrOpts as FluentBundle;
      if (!allBundles.includes(fb)) allBundles.push(fb);
      namespace = maybeNamespace;
    } else {
      const opts = namespaceOrFallbackOrOpts as CreateTranslatorOptions;
      if (opts.fallbackBundle && !allBundles.includes(opts.fallbackBundle)) {
        allBundles.push(opts.fallbackBundle);
      }
      if (opts.fallbackBundles) {
        const list = Array.isArray(opts.fallbackBundles)
          ? opts.fallbackBundles
          : [opts.fallbackBundles];
        for (const fb of list) {
          if (fb && !allBundles.includes(fb)) allBundles.push(fb);
        }
      }
      namespace = opts.namespace ?? maybeNamespace;
      debug = opts.debug ?? false;
    }
  } else {
    namespace = maybeNamespace;
  }

  const formatCandidate = (
    targetBundle: FluentBundle,
    candidate: string,
    args?: FluentArgs
  ): FormatCandidateResult => {
    const msg = targetBundle.getMessage(candidate);
    if (msg?.value) {
      const errors: Error[] = [];
      const formatted = targetBundle.formatPattern(msg.value, args, errors);
      if (errors.length > 0) {
        console.warn(`[next-fluent] Format errors for key "${candidate}":`, errors);
        // Never expose Fluent's partially formatted output. A message that exists
        // but cannot be formatted is a terminal resolution failure, matching the
        // Rust lenient path rather than silently falling through to another locale.
        return FORMAT_ERROR;
      }
      return formatted;
    }
    return null;
  };

  const tFn = (key: string, args?: FluentArgs): string => {
    const candidates = buildKeyCandidates(namespace, key);
    const fallbackKey = namespace ? `${namespace}.${key}` : key;
    for (const b of allBundles) {
      for (const candidate of candidates) {
        const formatted = formatCandidate(b, candidate, args);
        if (formatted === FORMAT_ERROR) return fallbackKey;
        if (formatted !== null) return formatted;
      }
    }

    if (debug) {
      console.warn(`[next-fluent] Missing translation for key "${fallbackKey}"`);
      return `[MISSING: ${fallbackKey}]`;
    }

    return fallbackKey;
  };

  const getRawValue = (targetBundle: FluentBundle, candidate: string): string[] | string | null => {
    const msg = targetBundle.getMessage(candidate);
    if (!msg) return null;

    if (msg.attributes && Object.keys(msg.attributes).length > 0) {
      const sortedAttrKeys = Object.keys(msg.attributes).sort((a, b) => {
        const numA = Number.parseInt(a.replace(/\D+/g, ''), 10);
        const numB = Number.parseInt(b.replace(/\D+/g, ''), 10);
        if (!Number.isNaN(numA) && !Number.isNaN(numB)) {
          return numA - numB;
        }
        return a.localeCompare(b);
      });

      return sortedAttrKeys.map((attrKey) => {
        const pattern = msg.attributes[attrKey];
        return targetBundle.formatPattern(pattern, undefined, []);
      });
    }

    if (msg.value) {
      const rawText = targetBundle.formatPattern(msg.value, undefined, []);
      const trimmed = rawText.trim();
      if (trimmed.startsWith('[') && trimmed.endsWith(']')) {
        try {
          const parsed = JSON.parse(trimmed);
          if (Array.isArray(parsed)) {
            return parsed.map(String);
          }
        } catch {
          // fallback to raw text
        }
      }
      return rawText;
    }

    return null;
  };

  tFn.raw = (key: string): string[] | string => {
    const candidates = buildKeyCandidates(namespace, key);
    for (const b of allBundles) {
      for (const candidate of candidates) {
        const res = getRawValue(b, candidate);
        if (res !== null) return res;
      }
    }

    const fallbackKey = namespace ? `${namespace}.${key}` : key;
    if (debug) {
      console.warn(`[next-fluent] Missing raw translation for key "${fallbackKey}"`);
      return `[MISSING: ${fallbackKey}]`;
    }

    return fallbackKey;
  };

  tFn.rich = (key: string, values?: RichTranslationValues): React.ReactNode => {
    const fluentArgs: FluentArgs = {};
    if (values) {
      for (const [k, v] of Object.entries(values)) {
        if (
          typeof v === 'string' ||
          typeof v === 'number' ||
          v instanceof Date ||
          (typeof v === 'object' && v !== null && 'type' in v)
        ) {
          fluentArgs[k] = v as FluentVariable;
        }
      }
    }

    const formattedText = tFn(key, fluentArgs);
    return parseRichText(formattedText, values);
  };

  tFn.has = (key: string): boolean => {
    const candidates = buildKeyCandidates(namespace, key);
    for (const candidate of candidates) {
      for (const b of allBundles) {
        if (b.hasMessage(candidate)) return true;
      }
    }
    return false;
  };

  return tFn as Translations;
}
