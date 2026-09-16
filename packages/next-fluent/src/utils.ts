const MAX_LOCALE_TAG_LENGTH = 64;

function localeDiagnosticValue(locale: string): string {
  if (locale.length > MAX_LOCALE_TAG_LENGTH) {
    return `<oversized locale: ${locale.length} code units>`;
  }
  return locale;
}

export function canonicalizeLocale(locale?: string | null): string | undefined {
  if (!locale || typeof locale !== 'string') return undefined;
  if (locale.length > MAX_LOCALE_TAG_LENGTH) return undefined;

  // Bound request-controlled raw input before trim/replaceAll can allocate copies.
  const raw = locale.trim();
  if (!raw) return undefined;
  const normalized = raw.replaceAll('_', '-');

  try {
    const canonical = Intl.getCanonicalLocales(normalized);
    return canonical[0];
  } catch {
    return undefined;
  }
}

export function normalizeLocaleTag(value?: string | null): string | undefined {
  return canonicalizeLocale(value);
}

export function matchSupportedLocale(
  value: string | null | undefined,
  locales: readonly string[]
): string | undefined {
  if (!value) return undefined;
  const canonical = canonicalizeLocale(value);
  if (!canonical) return undefined;

  // 1. Exact canonical match
  const exact = locales.find((loc) => {
    const locCanonical = canonicalizeLocale(loc);
    return locCanonical?.toLowerCase() === canonical.toLowerCase();
  });
  if (exact) return exact;

  // 2. Base language match (e.g. "en-US" -> "en")
  const baseLang = canonical.split('-')[0]?.toLowerCase();
  if (baseLang) {
    return locales.find((loc) => {
      const locCanonical = canonicalizeLocale(loc);
      return locCanonical?.toLowerCase() === baseLang || loc.toLowerCase() === baseLang;
    });
  }

  return undefined;
}

export function resolveAcceptLanguage(
  header: string | null | undefined,
  locales: readonly string[]
): string | undefined {
  if (!header) return undefined;

  const candidates = header
    .split(',')
    .map((entry) => {
      const trimmed = entry.trim();
      if (!trimmed) return null;

      const [tagPart, ...rest] = trimmed.split(';');
      const tag = tagPart.trim();
      if (!tag) return null;

      let quality = 1.0;
      for (const param of rest) {
        const match = param.trim().match(/^q\s*=\s*([0-9.]+)/i);
        if (match) {
          const parsed = Number.parseFloat(match[1]);
          quality = Number.isNaN(parsed) ? 1.0 : parsed;
          break;
        }
      }

      if (quality <= 0) return null;
      return { tag, quality };
    })
    .filter((item): item is { tag: string; quality: number } => item !== null)
    .sort((a, b) => b.quality - a.quality);

  for (const { tag } of candidates) {
    if (tag === '*') {
      return locales[0];
    }
    const matched = matchSupportedLocale(tag, locales);
    if (matched) return matched;
  }

  return undefined;
}

export function validateI18nConfig(options: {
  locales: readonly string[];
  defaultLocale: string;
}): void {
  if (!options || !Array.isArray(options.locales) || options.locales.length === 0) {
    throw new Error('[next-fluent] "locales" must be a non-empty array.');
  }

  const canonicalLocales = options.locales.map((loc) => {
    const canonical = canonicalizeLocale(loc);
    if (!canonical) {
      throw new Error(
        `[next-fluent] Invalid locale tag in "locales": "${localeDiagnosticValue(loc)}"`
      );
    }
    return canonical.toLowerCase();
  });

  const defaultCanonical = canonicalizeLocale(options.defaultLocale);
  if (!defaultCanonical) {
    throw new Error(
      `[next-fluent] Invalid "defaultLocale": "${localeDiagnosticValue(options.defaultLocale)}"`
    );
  }

  if (!canonicalLocales.includes(defaultCanonical.toLowerCase())) {
    throw new Error(
      `[next-fluent] "defaultLocale" ("${options.defaultLocale}") must be included in "locales" [${options.locales.join(', ')}].`
    );
  }
}

export function withKebabKey(key: string): string {
  return key.replaceAll('.', '-');
}

export function buildKeyCandidates(
  namespace: string | undefined,
  key: string
): string[] {
  const candidates: string[] = [];
  const pushCandidate = (candidate: string): void => {
    if (!candidates.includes(candidate)) {
      candidates.push(candidate);
    }
  };

  const cleanNs = namespace?.trim();
  if (cleanNs) {
    const joined = `${cleanNs}.${key}`;
    pushCandidate(withKebabKey(joined));
    pushCandidate(joined);

    // Keep the legacy namespace-hyphen alias, but avoid probing an equivalent
    // candidate twice when it matches the kebab form of the joined key.
    const nsHyphen = withKebabKey(cleanNs);
    if (nsHyphen !== cleanNs) {
      pushCandidate(`${nsHyphen}-${key}`);
    }
  }

  // Key as-is with kebab conversion and direct key, preserving first-seen order.
  pushCandidate(withKebabKey(key));
  pushCandidate(key);

  return candidates;
}
