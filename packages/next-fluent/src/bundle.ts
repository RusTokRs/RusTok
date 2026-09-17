import type { FluentBundle, FluentFunction } from '@fluent/bundle';
import type React from 'react';
import type { FluentArgs, FluentVariable, RichTranslationValues, Translations } from './types';
import { buildKeyCandidates, canonicalizeLocale } from './utils';
import { parseRichText } from './rich';
import {
  getCachedFluentBundle,
  clearBundleCache,
  getBundleCacheStats,
  type CreateFluentBundleOptions,
} from './cache';

export {
  getCachedFluentBundle,
  clearBundleCache,
  getBundleCacheStats,
  type CreateFluentBundleOptions,
};

function fluentBundleLocaleDiagnostic(locale: unknown): string {
  if (typeof locale !== 'string') {
    const kind = locale === null ? 'null' : typeof locale;
    return `<non-string locale: ${kind}>`;
  }
  if (locale.length > 64) {
    return `<oversized locale: ${locale.length} code units>`;
  }
  return locale;
}

export function createFluentBundle(
  locale: string,
  ftlSource: string | readonly string[],
  options: CreateFluentBundleOptions = {}
): FluentBundle {
  // Low-level bundle construction follows the same locale contract as the
  // high-level runtime: bound raw input, canonicalize aliases such as ru_RU,
  // and reject malformed identities before they reach Fluent or Intl helpers.
  const canonicalLocale = canonicalizeLocale(locale);
  if (!canonicalLocale) {
    throw new Error(
      `[next-fluent] Invalid Fluent bundle locale: "${fluentBundleLocaleDiagnostic(locale)}"`
    );
  }

  return getCachedFluentBundle(canonicalLocale, ftlSource, options);
}

export interface CreateTranslatorOptions {
  fallbackBundle?: FluentBundle | null;
  fallbackBundles?: FluentBundle | readonly FluentBundle[] | null;
  namespace?: string;
  debug?: boolean;
}

const FORMAT_ERROR = Symbol('format-error');
type FormatCandidateResult = string | null | typeof FORMAT_ERROR;
type RawCandidateResult = string[] | string | null | typeof FORMAT_ERROR;

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

  const getRawValue = (
    targetBundle: FluentBundle,
    candidate: string
  ): RawCandidateResult => {
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

      const values: string[] = [];
      for (const attrKey of sortedAttrKeys) {
        const pattern = msg.attributes[attrKey];
        const errors: Error[] = [];
        const formatted = targetBundle.formatPattern(pattern, undefined, errors);
        if (errors.length > 0) {
          console.warn(
            `[next-fluent] Format errors for raw attribute "${candidate}.${attrKey}":`,
            errors
          );
          return FORMAT_ERROR;
        }
        values.push(formatted);
      }
      return values;
    }

    if (msg.value) {
      const errors: Error[] = [];
      const rawText = targetBundle.formatPattern(msg.value, undefined, errors);
      if (errors.length > 0) {
        console.warn(`[next-fluent] Format errors for raw key "${candidate}":`, errors);
        return FORMAT_ERROR;
      }
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
    const fallbackKey = namespace ? `${namespace}.${key}` : key;
    for (const b of allBundles) {
      for (const candidate of candidates) {
        const res = getRawValue(b, candidate);
        if (res === FORMAT_ERROR) return fallbackKey;
        if (res !== null) return res;
      }
    }

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
