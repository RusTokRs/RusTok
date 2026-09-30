/**
 * Single JavaScript projection of the platform locale-tag contract.
 *
 * The canonical owner of locale parsing and CLDR canonicalization is
 * `crates/ui/rustok-ui-i18n` (ICU4X). Node-side gates cannot call it, so this
 * module exists for exactly one job: linting catalog **file names** and
 * reporting a canonical spelling. It must stay a structural check and must not
 * grow alias resolution, likely-subtag inference, or fallback logic — those
 * belong to the Rust owner.
 *
 * Previously `verify-ui-i18n-parity.mjs` carried a private copy of this logic
 * with a different length bound, which made it a fifth competing normalizer.
 */

/**
 * Raw-input bound shared with `rustok_ui_i18n::MAX_LOCALE_TAG_LEN`.
 * `rustok-api` additionally narrows the *canonical* identity to 32 bytes for
 * storage width; catalog file names are canonical identities, so the stricter
 * storage bound is the one that applies here.
 */
export const MAX_RAW_LOCALE_TAG_LEN = 64;
export const MAX_CANONICAL_LOCALE_TAG_LEN = 32;

/**
 * Canonicalizes the structural casing of a well-formed BCP-47 tag.
 *
 * Returns `null` when the value is not structurally well formed. A non-null
 * result is *not* a claim that the tag is registered or that ICU4X would
 * produce the same canonical form (aliases such as `iw` are deliberately not
 * resolved here).
 *
 * @param {string} value
 * @returns {string|null}
 */
export function canonicalizeLocaleTag(value) {
  if (typeof value !== "string" || value.length > MAX_RAW_LOCALE_TAG_LEN) {
    return null;
  }

  const normalized = value.trim().replaceAll("_", "-");
  if (!normalized) return null;

  const parts = normalized.split("-");
  if (parts.some((part) => part.length === 0)) return null;
  if (!/^[A-Za-z]{2,8}$/.test(parts[0])) return null;

  const rebuilt = [parts[0].toLowerCase()];
  for (const part of parts.slice(1)) {
    if (!/^[A-Za-z0-9]{1,8}$/.test(part)) return null;

    if (/^[A-Za-z]{4}$/.test(part)) {
      rebuilt.push(`${part[0].toUpperCase()}${part.slice(1).toLowerCase()}`);
      continue;
    }
    if (/^[A-Za-z]{2}$/.test(part) || /^\d{3}$/.test(part)) {
      rebuilt.push(/^\d{3}$/.test(part) ? part : part.toUpperCase());
      continue;
    }
    rebuilt.push(part.toLowerCase());
  }

  const result = rebuilt.join("-");
  return result.length <= MAX_CANONICAL_LOCALE_TAG_LEN ? result : null;
}

/**
 * Returns true when `value` is already spelled canonically.
 *
 * @param {string} value
 */
export function isCanonicalLocaleTag(value) {
  return canonicalizeLocaleTag(value) === value;
}
