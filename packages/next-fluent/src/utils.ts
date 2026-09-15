export function normalizeLocaleTag(value?: string | null): string | undefined {
  const normalized = value?.trim().replaceAll('_', '-');
  if (!normalized || normalized.length > 32) {
    return undefined;
  }

  const parts = normalized
    .split('-')
    .map((part) => part.trim())
    .filter(Boolean);

  if (parts.length === 0) {
    return undefined;
  }

  const rebuilt: string[] = [];
  for (const [index, part] of parts.entries()) {
    if (!/^[A-Za-z0-9]+$/.test(part)) {
      return undefined;
    }

    if (index === 0) {
      rebuilt.push(part.toLowerCase());
      continue;
    }

    if (/^[A-Za-z]{2}$/.test(part)) {
      rebuilt.push(part.toUpperCase());
      continue;
    }

    if (/^[A-Za-z]{4}$/.test(part)) {
      rebuilt.push(`${part[0].toUpperCase()}${part.slice(1).toLowerCase()}`);
      continue;
    }

    if (/^\d{3}$/.test(part)) {
      rebuilt.push(part);
      continue;
    }

    rebuilt.push(part.toLowerCase());
  }

  return rebuilt.join('-');
}

export function matchSupportedLocale(
  value: string | null | undefined,
  locales: readonly string[]
): string | undefined {
  if (!value) return undefined;
  const normalized = normalizeLocaleTag(value);
  if (!normalized) return undefined;

  const exact = locales.find(
    (loc) => loc.toLowerCase() === normalized.toLowerCase()
  );
  if (exact) return exact;

  const baseLang = normalized.split('-')[0].toLowerCase();
  return locales.find((loc) => loc.toLowerCase() === baseLang);
}

export function resolveAcceptLanguage(
  header: string | null | undefined,
  locales: readonly string[]
): string | undefined {
  if (!header) return undefined;

  const candidates = header
    .split(',')
    .map((entry) => {
      const [tag, qPart] = entry.split(';q=');
      const quality = qPart ? Number.parseFloat(qPart) : 1.0;
      return { tag: tag.trim(), quality: Number.isNaN(quality) ? 1.0 : quality };
    })
    .filter(({ tag }) => Boolean(tag))
    .sort((a, b) => b.quality - a.quality);

  for (const { tag } of candidates) {
    const matched = matchSupportedLocale(tag, locales);
    if (matched) return matched;
  }

  return undefined;
}

export function withKebabKey(key: string): string {
  return key.replaceAll('.', '-');
}

export function buildKeyCandidates(
  namespace: string | undefined,
  key: string
): string[] {
  const candidates: string[] = [];

  if (namespace && namespace.trim()) {
    const cleanNs = namespace.trim();
    const joined = `${cleanNs}.${key}`;
    candidates.push(withKebabKey(joined));
    candidates.push(joined);

    // Also support checking namespace with hyphen
    const nsHyphen = withKebabKey(cleanNs);
    if (nsHyphen !== cleanNs) {
      candidates.push(`${nsHyphen}-${key}`);
    }
  }

  // Key as-is with kebab conversion and direct key
  candidates.push(withKebabKey(key));
  if (!candidates.includes(key)) {
    candidates.push(key);
  }

  return candidates;
}
