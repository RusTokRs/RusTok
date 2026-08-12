export type RichTextDirection = 'ltr' | 'rtl' | 'auto';

export interface RichTextAuthoringContext {
  locale: string;
  direction: RichTextDirection;
  spellcheck: boolean;
}

export interface RichTextAuthoringContextInput {
  contentLocale: string;
  direction?: RichTextDirection;
  spellcheck?: boolean;
}

export function createRichTextAuthoringContext(
  input: RichTextAuthoringContextInput
): RichTextAuthoringContext {
  const locale = canonicalRichTextLocale(input.contentLocale);
  return {
    locale,
    direction: input.direction ?? richTextDirectionForLocale(locale),
    spellcheck: input.spellcheck ?? true
  };
}

export function isRichTextAuthoringContext(
  value: unknown
): value is RichTextAuthoringContext {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return false;
  }
  const context = value as Record<string, unknown>;
  return (
    typeof context.locale === 'string' &&
    canonicalRichTextLocale(context.locale) === context.locale &&
    (context.direction === 'ltr' ||
      context.direction === 'rtl' ||
      context.direction === 'auto') &&
    typeof context.spellcheck === 'boolean'
  );
}

export function canonicalRichTextLocale(value: string): string {
  const candidate = value.trim();
  if (!candidate || candidate.length > 64) return 'und';
  try {
    return Intl.getCanonicalLocales(candidate)[0] ?? 'und';
  } catch {
    return 'und';
  }
}

export function richTextDirectionForLocale(
  locale: string
): RichTextDirection {
  if (locale === 'und') return 'auto';
  try {
    const localeInfo = new Intl.Locale(locale) as Intl.Locale & {
      textInfo?: { direction?: string };
      getTextInfo?: () => { direction?: string };
    };
    const direction =
      localeInfo.textInfo?.direction ?? localeInfo.getTextInfo?.().direction;
    return direction === 'ltr' || direction === 'rtl' ? direction : 'auto';
  } catch {
    return 'auto';
  }
}
