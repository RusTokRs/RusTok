import { FluentBundle, FluentResource } from '@fluent/bundle';
import type { FluentArgs, Translations } from './types';
import { buildKeyCandidates } from './utils';

export function createFluentBundle(
  locale: string,
  ftlSource: string
): FluentBundle {
  const bundle = new FluentBundle(locale, { useIsolating: false });
  const resource = new FluentResource(ftlSource);
  const errors = bundle.addResource(resource);
  if (errors && errors.length > 0) {
    // Log warnings if any syntax issues in resource
    console.warn(`[next-fluent] Warnings adding FTL resource for locale ${locale}:`, errors);
  }
  return bundle;
}

export function createTranslator(
  bundle: FluentBundle | null,
  namespace?: string
): Translations {
  const tFn = (key: string, args?: FluentArgs): string => {
    if (!bundle) {
      return namespace ? `${namespace}.${key}` : key;
    }

    const candidates = buildKeyCandidates(namespace, key);
    for (const candidate of candidates) {
      const msg = bundle.getMessage(candidate);
      if (msg?.value) {
        const errors: Error[] = [];
        const formatted = bundle.formatPattern(msg.value, args, errors);
        if (errors.length > 0) {
          console.warn(`[next-fluent] Format errors for key "${candidate}":`, errors);
        }
        return formatted;
      }
    }

    return namespace ? `${namespace}.${key}` : key;
  };

  tFn.raw = (key: string): string[] | string => {
    if (!bundle) {
      return namespace ? `${namespace}.${key}` : key;
    }

    const candidates = buildKeyCandidates(namespace, key);
    for (const candidate of candidates) {
      const msg = bundle.getMessage(candidate);
      if (msg) {
        // If message has attributes (e.g. .0 = ..., .1 = ... for arrays/lists)
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
            return bundle.formatPattern(pattern, undefined, []);
          });
        }

        // If message has value, check if it's JSON array
        if (msg.value) {
          const rawText = bundle.formatPattern(msg.value, undefined, []);
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
      }
    }

    return namespace ? `${namespace}.${key}` : key;
  };

  return tFn as Translations;
}
