const RESERVED_SLUGS = new Set([
  'admin',
  'api',
  'null',
  'undefined',
  'new',
  'edit',
  'delete'
]);
const MAX_SLUG_CHARACTERS = 255;
const MAX_CATEGORY_ROUTE_BYTES = 120;
const MAX_CATEGORY_CODE_CHARACTERS = 128;

function canonicalSlug(value: string): string {
  const normalized = value.normalize('NFC').toLowerCase();
  let slug = Array.from(normalized)
    .filter((character) => /[\\p{L}\\p{N}\\- _]/u.test(character))
    .map((character) =>
      character === ' ' || character === '_' ? '-' : character
    )
    .join('');

  slug = slug.split('-').filter(Boolean).join('-');
  slug = Array.from(slug)
    .slice(0, MAX_SLUG_CHARACTERS)
    .join('')
    .replace(/^-+|-+$/g, '');

  if (RESERVED_SLUGS.has(slug)) return `${slug}-1`;
  return slug || 'untitled';
}

function utf8Length(value: string): number {
  let length = 0;
  for (const character of value) {
    const codePoint = character.codePointAt(0) ?? 0;
    length +=
      codePoint <= 0x7f
        ? 1
        : codePoint <= 0x7ff
          ? 2
          : codePoint <= 0xffff
            ? 3
            : 4;
  }
  return length;
}

function truncateUtf8(value: string, maximumBytes: number): string {
  let result = '';
  let length = 0;
  for (const character of value) {
    const characterLength = utf8Length(character);
    if (length + characterLength > maximumBytes) break;
    result += character;
    length += characterLength;
  }
  return result;
}

/** Mirrors the Product owner's Unicode slug rules and 120-byte route limit. */
export function slugifyCategoryName(value: string): string {
  const slug = canonicalSlug(value);
  if (utf8Length(slug) <= MAX_CATEGORY_ROUTE_BYTES) return slug;
  return canonicalSlug(truncateUtf8(slug, MAX_CATEGORY_ROUTE_BYTES));
}

function stableNameHash(value: string): string {
  let hash = 0x811c9dc5;
  for (const character of value.normalize('NFC')) {
    hash = Math.imul(hash ^ (character.codePointAt(0) ?? 0), 0x01000193);
  }
  return (hash >>> 0).toString(16).padStart(8, '0');
}

/** Produces a bounded lowercase-ASCII category code, including for Unicode names. */
export function categoryCodeFromName(value: string): string {
  if (!value.trim()) return '';
  const normalized = value.normalize('NFKD').toLowerCase();
  const ascii = normalized
    .replace(/[\\p{M}]/gu, '')
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '');

  if (ascii) {
    if (ascii.length <= MAX_CATEGORY_CODE_CHARACTERS) return ascii;
    const suffix = `_${stableNameHash(value)}`;
    return `${ascii.slice(0, MAX_CATEGORY_CODE_CHARACTERS - suffix.length).replace(/_+$/g, '')}${suffix}`;
  }

  const encoded = `u_${Array.from(value.normalize('NFC'))
    .map((character) => (character.codePointAt(0) ?? 0).toString(16))
    .join('_')}`;
  if (encoded.length <= MAX_CATEGORY_CODE_CHARACTERS) return encoded;
  return `category_${stableNameHash(value)}`;
}
