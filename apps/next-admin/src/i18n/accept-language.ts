/**
 * Bounded, q-aware `Accept-Language` parsing for the Next admin host.
 *
 * The canonical implementation of this contract is
 * `rustok_ui_i18n::try_parse_accept_language` (Rust). This file is the Node
 * projection of the *same* rules, not a second policy:
 *
 *   - the raw field value is bounded before any parsing work;
 *   - the number of comma-separated ranges is bounded;
 *   - q-values are exact integer thousandths, never floats;
 *   - a malformed individual range is ignored instead of discarding the
 *     later valid preferences;
 *   - `q=0` is an explicit rejection, not "lowest priority";
 *   - ordering is descending quality with stable source order for ties.
 *
 * The previous implementation split on `,`, dropped everything after `;` and
 * returned the first supported match, so `en;q=0.1, ru;q=0.9` resolved to `en`
 * and `en;q=0` resolved to `en`.
 *
 * Host policy (header vs cookie vs effective-locale header precedence,
 * supported-locale allowlist, final default) stays in `request.ts`.
 */

export const MAX_ACCEPT_LANGUAGE_LEN = 4096;
export const MAX_ACCEPT_LANGUAGE_RANGES = 64;

export interface AcceptLanguagePreference {
  /** Canonical language range, or `null` for the `*` wildcard. */
  readonly range: string | null;
  /** RFC quality as exact thousandths in `0..=1000`. */
  readonly qualityThousandths: number;
}

function parseQuality(value: string): number | null {
  if (value === "0") return 0;
  if (value === "1") return 1000;

  const dot = value.indexOf(".");
  if (dot < 0) return null;

  const whole = value.slice(0, dot);
  const fraction = value.slice(dot + 1);
  if (fraction.length > 3 || !/^\d*$/.test(fraction)) return null;

  if (whole === "0") {
    if (fraction.length === 0) return 0;
    return Number.parseInt(fraction.padEnd(3, "0"), 10);
  }
  if (whole === "1" && /^0*$/.test(fraction)) return 1000;
  return null;
}

function parseRange(raw: string): AcceptLanguagePreference | null {
  const segments = raw.trim().split(";");
  const range = segments[0]?.trim();
  if (!range) return null;
  if (range !== "*" && !/^[A-Za-z]{1,8}(-[A-Za-z0-9]{1,8})*$/.test(range)) {
    return null;
  }

  let qualityThousandths = 1000;
  let qualitySeen = false;

  for (const parameter of segments.slice(1)) {
    const trimmed = parameter.trim();
    const eq = trimmed.indexOf("=");
    if (eq < 0) {
      // A bare `q` parameter is malformed; drop the whole range.
      if (trimmed.toLowerCase() === "q") return null;
      continue;
    }
    if (trimmed.slice(0, eq).trim().toLowerCase() !== "q") continue;
    if (qualitySeen) return null;
    qualitySeen = true;

    const parsed = parseQuality(trimmed.slice(eq + 1).trim());
    if (parsed === null) return null;
    qualityThousandths = parsed;
  }

  return { range: range === "*" ? null : range, qualityThousandths };
}

/** Parses a bounded field value into quality-ordered preferences. */
export function parseAcceptLanguage(
  header: string | null | undefined,
): AcceptLanguagePreference[] {
  if (!header || header.length > MAX_ACCEPT_LANGUAGE_LEN) return [];

  const rawRanges = header.split(",");
  if (rawRanges.length > MAX_ACCEPT_LANGUAGE_RANGES) return [];

  return rawRanges
    .map((raw, sourceIndex) => {
      const preference = parseRange(raw);
      return preference === null ? null : { preference, sourceIndex };
    })
    .filter((entry): entry is { preference: AcceptLanguagePreference; sourceIndex: number } =>
      entry !== null,
    )
    .sort(
      (left, right) =>
        right.preference.qualityThousandths - left.preference.qualityThousandths ||
        left.sourceIndex - right.sourceIndex,
    )
    .map((entry) => entry.preference);
}

/** Returns accepted, non-rejected language ranges in quality order. */
export function acceptedLanguageRanges(
  header: string | null | undefined,
): string[] {
  const ranges: string[] = [];
  for (const preference of parseAcceptLanguage(header)) {
    if (preference.qualityThousandths === 0 || preference.range === null) continue;
    if (!ranges.includes(preference.range)) ranges.push(preference.range);
  }
  return ranges;
}
