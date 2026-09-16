const MAX_LOCALE_TAG_LENGTH = 64;
const HTTP_QVALUE = /^(?:0(?:\.\d{0,3})?|1(?:\.0{0,3})?)$/;

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

function localeLookupCandidates(canonical: string): string[] {
  const candidates: string[] = [];
  const pushCandidate = (candidate?: string): void => {
    if (candidate && !candidates.some((item) => item.toLowerCase() === candidate.toLowerCase())) {
      candidates.push(candidate);
    }
  };

  pushCandidate(canonical);

  try {
    const locale = new Intl.Locale(canonical);

    // Extensions are not a structural fallback layer for catalog selection.
    // Probe the extension-free base name before removing variant/core parts.
    pushCandidate(locale.baseName);

    // Treat all variants as one specificity layer. Intl canonicalization can
    // reorder variants, so peeling serialized subtags one-by-one can manufacture
    // arbitrary partial-variant parents just like unic-langid can on Rust.
    const core = [locale.language, locale.script, locale.region]
      .filter((part): part is string => Boolean(part))
      .join('-');
    pushCandidate(core);

    if (locale.region) {
      pushCandidate([locale.language, locale.script].filter(Boolean).join('-'));
    }
    if (locale.script || locale.region) {
      pushCandidate(locale.language);
    }
  } catch {
    // `canonical` came from Intl.getCanonicalLocales, so this is defensive only.
  }

  return candidates;
}

export function matchSupportedLocale(
  value: string | null | undefined,
  locales: readonly string[]
): string | undefined {
  if (!value) return undefined;
  const canonical = canonicalizeLocale(value);
  if (!canonical) return undefined;

  for (const candidate of localeLookupCandidates(canonical)) {
    const normalizedCandidate = candidate.toLowerCase();
    const matched = locales.find((loc) => {
      const locCanonical = canonicalizeLocale(loc);
      return locCanonical?.toLowerCase() === normalizedCandidate;
    });
    if (matched) return matched;
  }

  return undefined;
}

function parseAcceptLanguageEntry(entry: string): { tag: string; quality: number } | undefined {
  const trimmed = entry.trim();
  if (!trimmed) return undefined;

  const firstSeparator = trimmed.indexOf(';');
  const tag = (firstSeparator === -1 ? trimmed : trimmed.slice(0, firstSeparator)).trim();
  if (!tag) return undefined;

  let quality = 1.0;
  let paramStart = firstSeparator === -1 ? trimmed.length : firstSeparator + 1;

  while (paramStart < trimmed.length) {
    const nextSeparator = trimmed.indexOf(';', paramStart);
    const paramEnd = nextSeparator === -1 ? trimmed.length : nextSeparator;
    const param = trimmed.slice(paramStart, paramEnd).trim();
    const qParam = param.match(/^q\s*=\s*(.*)$/i);
    if (qParam) {
      const rawQuality = qParam[1].trim();
      if (!HTTP_QVALUE.test(rawQuality)) return undefined;
      quality = Number(rawQuality);
      break;
    }

    if (nextSeparator === -1) break;
    paramStart = nextSeparator + 1;
  }

  if (quality <= 0) return undefined;
  return { tag, quality };
}

export function resolveAcceptLanguage(
  header: string | null | undefined,
  locales: readonly string[]
): string | undefined {
  if (!header) return undefined;

  // Parse candidates one at a time instead of materializing and sorting the
  // entire request-controlled header. Tracking only the best supported match
  // preserves descending-q and stable first-seen semantics with bounded
  // auxiliary memory regardless of candidate count.
  let bestLocale: string | undefined;
  let bestQuality = Number.NEGATIVE_INFINITY;
  let entryStart = 0;

  while (entryStart <= header.length) {
    const separator = header.indexOf(',', entryStart);
    const entryEnd = separator === -1 ? header.length : separator;
    const candidate = parseAcceptLanguageEntry(header.slice(entryStart, entryEnd));

    if (candidate && candidate.quality > bestQuality) {
      const matched = candidate.tag === '*'
        ? locales[0]
        : matchSupportedLocale(candidate.tag, locales);

      if (matched) {
        bestLocale = matched;
        bestQuality = candidate.quality;
      }
    }

    if (separator === -1) break;
    entryStart = separator + 1;
  }

  return bestLocale;
}

export function validateI18nConfig(options: {
  locales: readonly string[];
  defaultLocale: string;
}): void {
  if (!options || !Array.isArray(options.locales) || options.locales.length === 0) {
    throw new Error('[next-fluent] "locales" must be a non-empty array.');
  }

  // Locale matching throughout the package is canonical and case-insensitive.
  // Reject duplicate canonical identities at configuration time instead of
  // allowing two raw spellings to compete for the same runtime locale.
  const canonicalLocales = new Set<string>();
  for (const loc of options.locales) {
    const canonical = canonicalizeLocale(loc);
    if (!canonical) {
      throw new Error(
        `[next-fluent] Invalid locale tag in "locales": "${localeDiagnosticValue(loc)}"`
      );
    }

    const identity = canonical.toLowerCase();
    if (canonicalLocales.has(identity)) {
      throw new Error(
        `[next-fluent] Duplicate locale identity in "locales": "${canonical}"`
      );
    }
    canonicalLocales.add(identity);
  }

  const defaultCanonical = canonicalizeLocale(options.defaultLocale);
  if (!defaultCanonical) {
    throw new Error(
      `[next-fluent] Invalid "defaultLocale": "${localeDiagnosticValue(options.defaultLocale)}"`
    );
  }

  if (!canonicalLocales.has(defaultCanonical.toLowerCase())) {
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
